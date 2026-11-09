//! Hunt Manager — Koordinasi Multi-Puzzle Hunter dengan PostgreSQL Integration

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use num_bigint::{BigUint, RandBigInt};
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::chain::check_puzzle_status;
use crate::db::{self, models::Puzzle};
use crate::kangaroo::{biguint_to_scalar_32, run_kangaroo_window};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HunterStatus {
    Idle,
    Running,
    Paused,
    SolvedByUs,
    SolvedByOther,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HunterState {
    pub puzzle_num: i32,
    pub status: HunterStatus,
    pub round: u64,
    pub total_keys_checked: u64,
    pub keys_per_second: f64,
    pub windows_scanned: u64,
    pub last_window_hex: String,
    pub balance_btc: f64,
    pub found_key_hex: Option<String>,
    pub found_wif: Option<String>,
    pub started_at: Option<String>,
    pub last_check_at: Option<String>,
    pub active_workers: usize,
}

impl HunterState {
    pub fn new(puzzle_num: i32) -> Self {
        Self {
            puzzle_num,
            status: HunterStatus::Idle,
            round: 0,
            total_keys_checked: 0,
            keys_per_second: 0.0,
            windows_scanned: 0,
            last_window_hex: String::new(),
            balance_btc: 0.0,
            found_key_hex: None,
            found_wif: None,
            started_at: None,
            last_check_at: None,
            active_workers: 1,
        }
    }
}

pub struct HuntManager {
    pub pool: PgPool,
    pub states: Arc<Mutex<HashMap<i32, HunterState>>>,
    pub stop_flags: Arc<Mutex<HashMap<i32, Arc<AtomicBool>>>>,
    pub ws_tx: tokio::sync::broadcast::Sender<String>,
}

impl HuntManager {
    pub fn new(pool: PgPool) -> Self {
        let (ws_tx, _) = tokio::sync::broadcast::channel(200);
        Self {
            pool,
            states: Arc::new(Mutex::new(HashMap::new())),
            stop_flags: Arc::new(Mutex::new(HashMap::new())),
            ws_tx,
        }
    }

    pub fn get_state(&self, puzzle_num: i32) -> Option<HunterState> {
        self.states.lock().unwrap().get(&puzzle_num).cloned()
    }

    pub fn get_all_states(&self) -> Vec<HunterState> {
        self.states.lock().unwrap().values().cloned().collect()
    }

    pub fn stop(&self, puzzle_num: i32) {
        if let Some(flag) = self.stop_flags.lock().unwrap().get(&puzzle_num) {
            flag.store(true, Ordering::Relaxed);
        }
        if let Some(state) = self.states.lock().unwrap().get_mut(&puzzle_num) {
            state.status = HunterStatus::Paused;
            if let Ok(msg) = serde_json::to_string(&*state) {
                let _ = self.ws_tx.send(msg);
            }
        }
    }

    pub fn stop_all(&self) {
        let nums: Vec<i32> = self.stop_flags.lock().unwrap().keys().cloned().collect();
        for n in nums {
            self.stop(n);
        }
    }

    /// Memulai hunting puzzle secara dinamis dari database PostgreSQL
    pub async fn start(
        &self,
        puzzle_num: i32,
        window_bits: u32,
        check_interval_secs: u64,
        destination_address: String,
    ) -> Result<(), String> {
        // Ambil data puzzle dari PostgreSQL
        let puzzle = db::get_puzzle_by_number(&self.pool, puzzle_num)
            .await
            .map_err(|e| format!("Database error: {e}"))?
            .ok_or(format!("Puzzle #{puzzle_num} tidak ditemukan di database"))?;

        if puzzle.is_solved {
            return Err(format!("Puzzle #{puzzle_num} sudah tercatat SOLVED di database"));
        }

        // Hentikan proses jika sedang berjalan
        self.stop(puzzle_num);
        tokio::time::sleep(Duration::from_millis(200)).await;

        let stop_flag = Arc::new(AtomicBool::new(false));
        self.stop_flags.lock().unwrap().insert(puzzle_num, stop_flag.clone());

        let now_str = chrono_now();
        let new_state = HunterState {
            puzzle_num,
            status: HunterStatus::Running,
            started_at: Some(now_str),
            ..HunterState::new(puzzle_num)
        };

        {
            let mut states = self.states.lock().unwrap();
            states.insert(puzzle_num, new_state.clone());
        }

        // Broadcast initial running state via WebSocket
        if let Ok(msg) = serde_json::to_string(&new_state) {
            let _ = self.ws_tx.send(msg);
        }

        let pool = self.pool.clone();
        let states = self.states.clone();
        let stop_flag_cloned = stop_flag.clone();
        let destination_address_cloned = destination_address.clone();
        let ws_tx_cloned = self.ws_tx.clone();

        // Spawn task background Tokio
        tokio::spawn(async move {
            run_hunt_loop(
                pool,
                puzzle,
                window_bits,
                check_interval_secs,
                destination_address_cloned,
                states,
                stop_flag_cloned,
                ws_tx_cloned,
            )
            .await;
        });

        Ok(())
    }
}

/// Menghitung jumlah worker thread optimal secara dinamis.
/// Membagi kapasitas core CPU yang tersedia di antara puzzle yang sedang berstatus Running.
fn get_optimal_workers(states: &Arc<Mutex<HashMap<i32, HunterState>>>) -> usize {
    let total_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);

    let running_count = {
        let s = states.lock().unwrap();
        s.values()
            .filter(|st| st.status == HunterStatus::Running)
            .count()
    }
    .max(1);

    // Sisakan 2 thread untuk async Tokio, PostgreSQL, WebSocket stream, dan OS
    let compute_budget = total_cpus.saturating_sub(2).max(1);

    // Alokasikan thread per puzzle (minimal 1, maksimal 8 per puzzle agar tidak jenuh)
    let per_puzzle = compute_budget / running_count;
    per_puzzle.clamp(1, 8)
}

async fn run_hunt_loop(
    pool: PgPool,
    puzzle: Puzzle,
    window_bits: u32,
    check_interval_secs: u64,
    destination_address: String,
    states: Arc<Mutex<HashMap<i32, HunterState>>>,
    stop_flag: Arc<AtomicBool>,
    ws_tx: tokio::sync::broadcast::Sender<String>,
) {
    let puzzle_num = puzzle.puzzle_number;

    // Deteksi mode: Public Key Exposed (Kangaroo O(√N)) vs Address Scanner (Hash160)
    let pub_clean = puzzle.pubkey_hex.trim().trim_start_matches("0x");
    let target_pub_bytes: Option<[u8; 33]> = if pub_clean.len() == 66 {
        match hex::decode(pub_clean) {
            Ok(b) if b.len() == 33 => {
                let mut arr = [0u8; 33];
                arr.copy_from_slice(&b);
                Some(arr)
            }
            _ => None,
        }
    } else {
        None
    };

    let target_hash160: Option<[u8; 20]> = if target_pub_bytes.is_none() {
        match crate::kangaroo::address_to_hash160(&puzzle.address) {
            Ok(h) => Some(h),
            Err(e) => {
                update_status(&states, puzzle_num, HunterStatus::Error(format!("Alamat target tidak valid: {e}")));
                return;
            }
        }
    } else {
        None
    };

    // Mode Kangaroo menggunakan window_bits standar (default 30)
    // Mode Address Scanner menggunakan rentang window lebih rapat (maks 18 = 262K keys) agar round responsif
    let effective_window_bits = if target_pub_bytes.is_some() {
        window_bits
    } else {
        window_bits.min(18)
    };

    // Parse rentang awal dan akhir dari database
    let s_clean = puzzle.range_start_hex.trim().trim_start_matches("0x");
    let e_clean = puzzle.range_end_hex.trim().trim_start_matches("0x");
    let full_start = BigUint::parse_bytes(s_clean.as_bytes(), 16)
        .unwrap_or_else(|| BigUint::one() << (puzzle.bits as usize - 1));
    let full_end = BigUint::parse_bytes(e_clean.as_bytes(), 16)
        .unwrap_or_else(|| (BigUint::one() << puzzle.bits as usize) - BigUint::one());

    let window_size = BigUint::one() << effective_window_bits as usize;
    if full_end <= full_start || (full_end.clone() - full_start.clone()) < window_size {
        update_status(&states, puzzle_num, HunterStatus::Error("Rentang puzzle terlalu kecil".into()));
        return;
    }

    let max_window_id = (full_end.clone() - full_start.clone()) / window_size.clone();
    let dp_bits = (effective_window_bits / 4).clamp(4, 14);

    // Muat riwayat window yang sudah pernah diperiksa dari PostgreSQL
    let mut scanned: Vec<(BigUint, BigUint)> = db::load_scanned_windows(&pool, puzzle_num)
        .await
        .unwrap_or_default();

    // Pengecekan on-chain awal
    let onchain = check_puzzle_status(&puzzle.address).await;
    update_balance(&states, puzzle_num, onchain.balance_sats);

    if onchain.is_solved {
        let _ = db::mark_puzzle_solved(&pool, puzzle_num, &onchain.message).await;
        update_status(&states, puzzle_num, HunterStatus::SolvedByOther);
        return;
    }

    let mut round: u64 = 0;
    let mut total_keys: u64 = 0;
    let mut last_check = Instant::now();
    let session_start = Instant::now();

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            update_status(&states, puzzle_num, HunterStatus::Paused);
            break;
        }

        // Tentukan jumlah worker thread dinamis (otomatis adaptif jika hanya run 1 atau 2 puzzle)
        let num_workers = get_optimal_workers(&states);

        // Ambil batch window acak sebanyak num_workers tanpa duplikasi
        let mut batch_windows = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            let (window_id, w_start, w_end) = pick_random_window(
                &full_start,
                &full_end,
                &window_size,
                &max_window_id,
                &scanned,
            );
            insert_sorted(&mut scanned, (w_start.clone(), w_end.clone()));
            batch_windows.push((window_id, w_start, w_end));
        }

        round += num_workers as u64;

        let first_start_hex = format!("0x{:x}", batch_windows[0].1);
        let first_end_hex = format!("0x{:x}", batch_windows[0].2);

        // Update status in-memory untuk live dashboard
        {
            let mut s = states.lock().unwrap();
            if let Some(st) = s.get_mut(&puzzle_num) {
                st.last_window_hex = if num_workers > 1 {
                    format!("{first_start_hex}..{first_end_hex} (⚡ {num_workers} threads)")
                } else {
                    format!("{first_start_hex}..{first_end_hex}")
                };
                st.round = round;
                st.active_workers = num_workers;
            }
        }

        // Jalankan seluruh worker secara paralel di background thread pool
        let tasks = batch_windows.into_iter().map(|(win_id, ws, we)| {
            let stop_inner = stop_flag.clone();
            let target_pub = target_pub_bytes;
            let target_h160 = target_hash160;
            let ws_clone = ws.clone();
            let we_clone = we.clone();

            tokio::task::spawn_blocking(move || {
                let res = if let Some(pub_bytes) = target_pub {
                    run_kangaroo_window(
                        &pub_bytes,
                        &ws_clone,
                        &we_clone,
                        dp_bits,
                        Some(stop_inner),
                    )
                } else if let Some(h160) = target_h160 {
                    crate::kangaroo::run_address_window(
                        &h160,
                        &ws_clone,
                        &we_clone,
                        Some(stop_inner),
                    )
                } else {
                    crate::kangaroo::KangarooResult::miss(0, 0.0)
                };
                (win_id, ws, we, res)
            })
        });

        let batch_results = futures_util::future::join_all(tasks).await;

        let mut hit_result = None;
        let mut batch_keys: u64 = 0;

        for task_res in batch_results {
            let (win_id, ws, we, res) = match task_res {
                Ok(tuple) => tuple,
                Err(_) => continue,
            };

            batch_keys += res.keys_checked;

            // Simpan window yang discan ke PostgreSQL
            let s_hex = format!("0x{:x}", ws);
            let e_hex = format!("0x{:x}", we);
            let _ = db::record_scanned_window(
                &pool,
                puzzle_num,
                win_id,
                &s_hex,
                &e_hex,
                res.keys_checked as i64,
                res.elapsed_secs,
            )
            .await;

            if res.found && hit_result.is_none() {
                hit_result = Some(res);
            }
        }

        total_keys += batch_keys;
        let elapsed_total = session_start.elapsed().as_secs_f64();
        let kps = if elapsed_total > 0.0 {
            total_keys as f64 / elapsed_total
        } else {
            0.0
        };

        // Update stats in-memory
        {
            let mut s = states.lock().unwrap();
            if let Some(st) = s.get_mut(&puzzle_num) {
                st.total_keys_checked = total_keys;
                st.keys_per_second = kps;
                st.windows_scanned = scanned.len() as u64;
                st.last_check_at = Some(chrono_now());
                st.active_workers = num_workers;

                // Broadcast live progress ke WebSocket client
                if let Ok(msg) = serde_json::to_string(&*st) {
                    let _ = ws_tx.send(msg);
                }
            }
        }

        // JIKA KUNCI DITEMUKAN OLEH SALAH SATU WORKER!
        if let Some(result) = hit_result {
            if let Some(key_biguint) = result.found_key_biguint {
                let priv_bytes = biguint_to_scalar_32(&key_biguint);
                let priv_hex = hex::encode(priv_bytes);
                let priv_dec = key_biguint.to_string();
                let wif = priv_to_wif(&priv_bytes);

                // 1. Eksekusi Auto-Transfer Instan ke Dompet Tujuan
                let txid = match crate::transfer::execute_auto_transfer(
                    puzzle_num,
                    &priv_bytes,
                    &puzzle.address,
                    &destination_address,
                )
                .await
                {
                    Ok(tx) => {
                        tracing::info!("🚀 AUTO-TRANSFER BERHASIL! TxID: {tx}");
                        Some(tx)
                    }
                    Err(e) => {
                        tracing::error!("⚠️ Auto-transfer terkendala: {e}");
                        None
                    }
                };

                // 2. Simpan ke PostgreSQL
                let _ = db::save_found_key(
                    &pool,
                    puzzle_num,
                    &puzzle.address,
                    &priv_hex,
                    &priv_dec,
                    &wif,
                    txid.as_deref(),
                )
                .await;

                // Simpan backup fisik
                save_backup_file(puzzle_num, &priv_hex, &priv_dec, &wif, &puzzle.address);

                // 4. Kirim Notifikasi Instan ke Telegram
                let _ = crate::notify::send_telegram_alert(
                    puzzle_num,
                    puzzle.bits,
                    &puzzle.address,
                    &priv_hex,
                    &priv_dec,
                    &wif,
                    txid.as_deref(),
                    None,
                )
                .await;

                {
                    let mut s = states.lock().unwrap();
                    if let Some(st) = s.get_mut(&puzzle_num) {
                        st.status = HunterStatus::SolvedByUs;
                        st.found_key_hex = Some(priv_hex.clone());
                        st.found_wif = Some(wif.clone());

                        if let Ok(msg) = serde_json::to_string(&*st) {
                            let _ = ws_tx.send(msg);
                        }
                    }
                }

                tracing::info!("🎉🎉🎉 PUZZLE #{} SOLVED! KEY: 0x{}", puzzle_num, priv_hex);
                break;
            }
        }

        // On-chain Watchdog berkala
        if last_check.elapsed().as_secs() >= check_interval_secs {
            last_check = Instant::now();
            let onchain = check_puzzle_status(&puzzle.address).await;
            update_balance(&states, puzzle_num, onchain.balance_sats);
            if onchain.is_solved {
                let _ = db::mark_puzzle_solved(&pool, puzzle_num, &onchain.message).await;
                update_status(&states, puzzle_num, HunterStatus::SolvedByOther);
                tracing::warn!("🛑 Puzzle #{} telah solved oleh orang lain di on-chain!", puzzle_num);
                break;
            }
        }
    }
}

// ===== Utility Helpers =====

fn pick_random_window(
    full_start: &BigUint,
    full_end: &BigUint,
    window_size: &BigUint,
    max_window_id: &BigUint,
    scanned: &[(BigUint, BigUint)],
) -> (i64, BigUint, BigUint) {
    let mut rng = rand::thread_rng();
    loop {
        let cand: BigUint = rng.gen_biguint_range(&BigUint::zero(), &(max_window_id + BigUint::one()));
        let ws = full_start + (&cand * window_size);
        let we = (&ws + window_size - BigUint::one()).min(full_end.clone());
        if !is_range_scanned(&ws, &we, scanned) {
            let wid = cand.to_u64_digits().first().copied().unwrap_or(0) as i64;
            return (wid, ws, we);
        }
    }
}

fn update_status(states: &Arc<Mutex<HashMap<i32, HunterState>>>, puzzle_num: i32, status: HunterStatus) {
    if let Some(st) = states.lock().unwrap().get_mut(&puzzle_num) {
        st.status = status;
    }
}

fn update_balance(states: &Arc<Mutex<HashMap<i32, HunterState>>>, puzzle_num: i32, sats: i64) {
    if let Some(st) = states.lock().unwrap().get_mut(&puzzle_num) {
        st.balance_btc = sats.max(0) as f64 / 100_000_000.0;
    }
}

fn chrono_now() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn is_range_scanned(start: &BigUint, end: &BigUint, intervals: &[(BigUint, BigUint)]) -> bool {
    if intervals.is_empty() {
        return false;
    }
    let idx = intervals.partition_point(|(s, _)| s <= start);
    if idx > 0 {
        let (_, ref e) = intervals[idx - 1];
        if e >= start {
            return true;
        }
    }
    if idx < intervals.len() {
        let (ref s, _) = intervals[idx];
        if s <= end {
            return true;
        }
    }
    false
}

fn insert_sorted(intervals: &mut Vec<(BigUint, BigUint)>, item: (BigUint, BigUint)) {
    let pos = intervals.partition_point(|(s, _)| s < &item.0);
    intervals.insert(pos, item);
}

pub fn priv_to_wif(priv_bytes: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    let mut payload = Vec::with_capacity(38);
    payload.push(0x80); // mainnet private key prefix
    payload.extend_from_slice(priv_bytes);
    payload.push(0x01); // compressed public key flag

    let hash1 = Sha256::digest(&payload);
    let hash2 = Sha256::digest(&hash1);
    payload.extend_from_slice(&hash2[..4]); // 4-byte checksum

    bs58::encode(payload).into_string()
}

fn save_backup_file(puzzle_num: i32, priv_hex: &str, priv_dec: &str, wif: &str, address: &str) {
    use std::io::Write;
    let content = format!(
        "====================================================================\n\
         🎉 BITCOIN PUZZLE #{puzzle_num} SOLVED! (POLLARD KANGAROO — RUST) 🎉\n\
         ====================================================================\n\
         Waktu           : {}\n\
         Bitcoin Address : {address}\n\
         Private Key Hex : 0x{priv_hex}\n\
         Private Key Dec : {priv_dec}\n\
         Private Key WIF : {wif}\n\
         ====================================================================\n",
        chrono_now()
    );

    let fname = format!("FOUND_BTC_PUZZLE_{puzzle_num}.txt");
    if let Ok(mut f) = std::fs::File::create(&fname) {
        let _ = f.write_all(content.as_bytes());
    }

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("FOUND_KEYS_BACKUP.txt")
    {
        let _ = writeln!(f, "\n{content}");
    }
}
