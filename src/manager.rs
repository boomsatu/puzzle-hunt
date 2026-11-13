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
    Pausing,
    Paused,
    SolvedByUs,
    SolvedByOther,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceMode {
    Local,
    Vps,
    Gpu,
}

impl DeviceMode {
    pub fn from_env() -> Self {
        let val = std::env::var("HUNT_MODE")
            .or_else(|_| std::env::var("DEVICE_MODE"))
            .unwrap_or_else(|_| "local".to_string())
            .to_lowercase();

        match val.trim() {
            "vps" => DeviceMode::Vps,
            "gpu" | "cuda" => DeviceMode::Gpu,
            _ => DeviceMode::Local,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceMode::Local => "local",
            DeviceMode::Vps => "vps",
            DeviceMode::Gpu => "gpu",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerWindow {
    pub worker_id: usize,
    pub start_hex: String,
    pub end_hex: String,
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
    #[serde(default)]
    pub active_windows: Vec<WorkerWindow>,
    #[serde(default)]
    pub device_mode: String,
    #[serde(default)]
    pub gpu_info: Option<crate::kangaroo::cuda::GpuTelemetry>,
    #[serde(default)]
    pub window_bits: u32,
    #[serde(default)]
    pub window_keys: u64,
    #[serde(default)]
    pub keys_per_round: u64,
    #[serde(default)]
    pub is_kangaroo_mode: bool,
}

impl HunterState {
    pub fn new(puzzle_num: i32) -> Self {
        let dev_mode = DeviceMode::from_env();
        let gpu_info = if dev_mode == DeviceMode::Gpu {
            Some(crate::kangaroo::cuda::get_telemetry(16))
        } else {
            None
        };

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
            active_windows: Vec::new(),
            device_mode: dev_mode.as_str().to_string(),
            gpu_info,
            window_bits: 0,
            window_keys: 0,
            keys_per_round: 0,
            is_kangaroo_mode: false,
        }
    }
}

pub struct HuntManager {
    pub pool: PgPool,
    pub states: Arc<Mutex<HashMap<i32, HunterState>>>,
    pub stop_flags: Arc<Mutex<HashMap<i32, Arc<AtomicBool>>>>,
    pub pause_flags: Arc<Mutex<HashMap<i32, Arc<AtomicBool>>>>,
    pub ws_tx: tokio::sync::broadcast::Sender<String>,
    pub monitor: Arc<crate::monitor::SystemMonitor>,
}

impl HuntManager {
    pub fn new(pool: PgPool) -> Self {
        let (ws_tx, _) = tokio::sync::broadcast::channel(200);
        let monitor = Arc::new(crate::monitor::SystemMonitor::new());

        // Background task non-blocking untuk memantau & broadcast live CPU load & RAM
        // Dijalankan dengan interval 1500ms agar beban CPU sampling mendekati 0 (< 0.01% CPU)
        let monitor_clone = monitor.clone();
        let ws_tx_clone = ws_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1500));
            interval.tick().await; // Tick awal langsung selesai
            loop {
                interval.tick().await;
                let metrics = monitor_clone.sample();
                // Broadcast ke semua client WebSocket yang aktif
                if ws_tx_clone.receiver_count() > 0 {
                    if let Ok(msg) = serde_json::to_string(&metrics) {
                        let _ = ws_tx_clone.send(msg);
                    }
                }
            }
        });

        Self {
            pool,
            states: Arc::new(Mutex::new(HashMap::new())),
            stop_flags: Arc::new(Mutex::new(HashMap::new())),
            pause_flags: Arc::new(Mutex::new(HashMap::new())),
            ws_tx,
            monitor,
        }
    }

    pub fn get_state(&self, puzzle_num: i32) -> Option<HunterState> {
        self.states.lock().unwrap().get(&puzzle_num).cloned()
    }

    pub fn get_all_states(&self) -> Vec<HunterState> {
        self.states.lock().unwrap().values().cloned().collect()
    }

    /// Jeda cerdas (Smart Graceful Pause):
    /// Memberi sinyal agar worker menyelesaikan komputasi window aktif dan menyimpan
    /// seluruh riwayat ke PostgreSQL sebelum proses benar-benar dijeda.
    pub fn smart_pause(&self, puzzle_num: i32) {
        if let Some(flag) = self.pause_flags.lock().unwrap().get(&puzzle_num) {
            flag.store(true, Ordering::Relaxed);
        }
        if let Some(state) = self.states.lock().unwrap().get_mut(&puzzle_num) {
            if state.status == HunterStatus::Running {
                state.status = HunterStatus::Pausing;
                if let Ok(msg) = serde_json::to_string(&*state) {
                    let _ = self.ws_tx.send(msg);
                }
            }
        }
    }

    /// Hard Stop darurat (langsung hentikan worker jika sangat mendesak)
    pub fn force_stop(&self, puzzle_num: i32) {
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

    /// Default stop menggunakan Smart Pause agar tidak merusak data window
    pub fn stop(&self, puzzle_num: i32) {
        self.smart_pause(puzzle_num);
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
        self.force_stop(puzzle_num);
        tokio::time::sleep(Duration::from_millis(150)).await;

        let stop_flag = Arc::new(AtomicBool::new(false));
        self.stop_flags.lock().unwrap().insert(puzzle_num, stop_flag.clone());

        let pause_flag = Arc::new(AtomicBool::new(false));
        self.pause_flags.lock().unwrap().insert(puzzle_num, pause_flag.clone());

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
        let pause_flag_cloned = pause_flag.clone();
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
                pause_flag_cloned,
                stop_flag_cloned,
                ws_tx_cloned,
            )
            .await;
        });

        Ok(())
    }
}

/// Menghitung jumlah worker thread optimal secara dinamis berdasarkan konfigurasi DeviceMode:
/// - Local: Sisakan 2 core untuk OS, desktop, Tokio, PostgreSQL (max 8 thread per puzzle)
/// - VPS  : Headless Linux, maksimalkan core untuk puzzle dan sisakan HANYA 1 core untuk OS + PostgreSQL + Web API
/// - GPU  : Mode akselerasi CUDA, delegasikan komputasi ke GPU
fn get_optimal_workers(states: &Arc<Mutex<HashMap<i32, HunterState>>>, mode: DeviceMode) -> usize {
    let total_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);

    let running_count = {
        let s = states.lock().unwrap();
        s.values()
            .filter(|st| st.status == HunterStatus::Running || st.status == HunterStatus::Pausing)
            .count()
    }
    .max(1);

    match mode {
        DeviceMode::Local => {
            // Mode Local: Sisakan 2 core untuk OS + Postgres + UI desktop (aman & stabil)
            let compute_budget = total_cpus.saturating_sub(2).max(1);
            let per_puzzle = compute_budget / running_count;
            per_puzzle.clamp(1, 8)
        }
        DeviceMode::Vps => {
            // Mode VPS: Sisakan hanya 1 core untuk OS + PostgreSQL + Web API
            // Seluruh sisa core dimaksimalkan untuk puzzle (misal di VPS 8 core: 8 - 1 = 7 core penuh)
            let compute_budget = total_cpus.saturating_sub(1).max(1);
            let per_puzzle = compute_budget / running_count;
            per_puzzle.max(1)
        }
        DeviceMode::Gpu => {
            // Mode GPU: 16 worker windows per round — maksimalkan coverage posisi random!
            // CUDA driver men-serialize proses via time-slicing (1 aktif di GPU per waktu).
            // 16 lokasi jump berbeda per ronde = probabilitas menemukan key lebih besar.
            // Total: 16 × 2^30 = 17.18B keys/round (~2.3s/round @8B k/s)
            16
        }
    }
}

async fn run_hunt_loop(
    pool: PgPool,
    puzzle: Puzzle,
    window_bits: u32,
    check_interval_secs: u64,
    destination_address: String,
    states: Arc<Mutex<HashMap<i32, HunterState>>>,
    pause_flag: Arc<AtomicBool>,
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

    // Deteksi device mode seawal mungkin untuk menentukan parameter window optimal
    let device_mode = DeviceMode::from_env();

    // Hitung window bits optimal berdasarkan device mode dan tipe puzzle
    let total_range_bits = if puzzle.bits > 2 { (puzzle.bits - 1) as u32 } else { 1 };
    let effective_window_bits = match device_mode {
        DeviceMode::Gpu => {
            // GPU: 8 worker × 2^30 = 8.59B keys/round — CUDA driver serialize via time-slicing
            // Window 2^30 = 1.073.741.824 keys per worker — sweet spot: ~134ms per window @8B k/s
            // Total round: 8 × ~140ms = ~1,120ms (GPU utilization ~95%)
            // Ini memberikan 8 posisi jump berbeda per ronde = coverage menyebar!
            30u32.min(total_range_bits.saturating_sub(1).max(8))
        }
        _ => {
            if target_pub_bytes.is_some() {
                // Kangaroo mode (CPU): window_bits default 30 (1.073.741.824 keys)
                if total_range_bits > 30 {
                    window_bits.min(total_range_bits - 1)
                } else {
                    total_range_bits.saturating_sub(1).max(8)
                }
            } else {
                // Address scanner (CPU): cap at 18 bits (~262K keys/worker)
                window_bits.min(18).min(total_range_bits.saturating_sub(1).max(8))
            }
        }
    };

    // Parse rentang awal dan akhir dari database
    let s_clean = puzzle.range_start_hex.trim().trim_start_matches("0x");
    let e_clean = puzzle.range_end_hex.trim().trim_start_matches("0x");
    let full_start = BigUint::parse_bytes(s_clean.as_bytes(), 16)
        .unwrap_or_else(|| BigUint::one() << (puzzle.bits as usize - 1));
    let full_end = BigUint::parse_bytes(e_clean.as_bytes(), 16)
        .unwrap_or_else(|| (BigUint::one() << puzzle.bits as usize) - BigUint::one());

    let window_size = BigUint::one() << effective_window_bits as usize;
    if full_end <= full_start {
        update_status(&states, puzzle_num, HunterStatus::Error("Rentang puzzle tidak valid (end <= start)".into()));
        return;
    }

    // Auto-Padding: Jika total keyspace tidak habis dibagi window_size (sisa > 0),
    // genapkan secara otomatis dengan menambahkan padding pada batas akhir (effective_end)
    // sehingga sisa pembagian SELALU 0 dan seluruh worker memproses window dengan ukuran simetris.
    let raw_total_keys = (&full_end - &full_start) + BigUint::one();
    let remainder = &raw_total_keys % &window_size;
    let effective_end = if remainder.is_zero() {
        full_end.clone()
    } else {
        let padding = &window_size - &remainder;
        let padded_end = &full_end + &padding;
        tracing::info!(
            "📐 [AUTO-PADDING] Puzzle #{}: Rentang digenapkan (+{} keys) agar membagi tepat window 2^{} dengan sisa 0.",
            puzzle_num,
            padding,
            effective_window_bits
        );
        padded_end
    };

    let total_windows = (&effective_end - &full_start + BigUint::one()) / window_size.clone();
    let max_window_id = total_windows - BigUint::one();
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

    match device_mode {
        DeviceMode::Local => {
            tracing::info!("🖥️ [SETUP LOCAL] Mode Local aktif: Sisakan 2 core untuk OS, Postgres & desktop.");
        }
        DeviceMode::Vps => {
            tracing::info!("⚡ [SETUP VPS] Mode VPS aktif: Memaksimalkan core untuk puzzle (Sisakan hanya 1 core untuk OS + PostgreSQL + Web API).");
        }
        DeviceMode::Gpu => {
            let (cuda_ok, cuda_info) = crate::kangaroo::cuda::init_cuda_status();
            if cuda_ok {
                tracing::info!("🚀 [SETUP GPU] Mode GPU CUDA aktif: {cuda_info}");
            } else {
                tracing::warn!("⚠️ [SETUP GPU] CUDA solver binary tidak ditemukan — fallback ke Rust CPU engine (sangat lambat dengan window 2^{}).", effective_window_bits);
            }
            let gpu_window_keys: u64 = 1u64 << effective_window_bits;
            let gpu_total_round = gpu_window_keys * 16; // 16 workers
            tracing::info!(
                "🎯 [GPU CONFIG] 16 workers × 2^{} = {} keys/worker, total {}/round (~{:.1}s/round est. @8B k/s)",
                effective_window_bits, gpu_window_keys, gpu_total_round, gpu_total_round as f64 / 8_000_000_000.0
            );
        }
    }

    loop {
        // Cek sinyal jeda cerdas (Smart Pause) atau paksa stop sebelum memulai batch baru
        if pause_flag.load(Ordering::Relaxed) || stop_flag.load(Ordering::Relaxed) {
            tracing::info!("⏸️ [SMART PAUSE] Hunter puzzle #{puzzle_num} dijeda secara aman.");
            update_status(&states, puzzle_num, HunterStatus::Paused);
            if let Some(st) = states.lock().unwrap().get(&puzzle_num) {
                if let Ok(msg) = serde_json::to_string(st) {
                    let _ = ws_tx.send(msg);
                }
            }
            break;
        }

        // Tentukan jumlah worker thread dinamis sesuai device mode (local, vps, gpu)
        let num_workers = get_optimal_workers(&states, device_mode);

        // Ambil batch window dengan prioritas: WAJIB scan range awal terlebih dahulu, baru random jump!
        let mut batch_windows = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            let (window_id, w_start, w_end) = pick_window_smart(
                &full_start,
                &effective_end,
                &window_size,
                &max_window_id,
                &scanned,
                num_workers as u64,
            );
            insert_sorted(&mut scanned, (w_start.clone(), w_end.clone()));
            batch_windows.push((window_id, w_start, w_end));
        }

        round += num_workers as u64;

        let first_start_hex = format!("0x{:x}", batch_windows[0].1);
        let first_end_hex = format!("0x{:x}", batch_windows[0].2);

        let active_win_list: Vec<WorkerWindow> = batch_windows
            .iter()
            .enumerate()
            .map(|(idx, (_, ws, we))| WorkerWindow {
                worker_id: idx + 1,
                start_hex: format!("0x{:x}", ws),
                end_hex: format!("0x{:x}", we),
            })
            .collect();

        // Update status in-memory untuk live dashboard
        {
            let mut s = states.lock().unwrap();
            if let Some(st) = s.get_mut(&puzzle_num) {
                st.last_window_hex = if device_mode == DeviceMode::Gpu {
                    format!("{first_start_hex}..{first_end_hex} (⚡ {num_workers} GPU Workers 2^{})", effective_window_bits)
                } else if num_workers > 1 {
                    format!("{first_start_hex}..{first_end_hex} (⚡ {num_workers} threads)")
                } else {
                    format!("{first_start_hex}..{first_end_hex}")
                };
                st.round = round;
                st.active_workers = num_workers;
                st.active_windows = active_win_list.clone();
                st.device_mode = device_mode.as_str().to_string();
                let win_keys = 1u64 << effective_window_bits;
                st.window_bits = effective_window_bits;
                st.window_keys = win_keys;
                st.keys_per_round = (num_workers as u64) * win_keys;
                st.is_kangaroo_mode = target_pub_bytes.is_some();
                if device_mode == DeviceMode::Gpu {
                    st.gpu_info = Some(crate::kangaroo::cuda::get_telemetry(num_workers));
                }

                // Broadcast live update awal ronde agar posisi jump per core langsung bergerak di UI
                if let Ok(msg) = serde_json::to_string(&*st) {
                    let _ = ws_tx.send(msg);
                }
            }
        }

        // Jalankan seluruh worker secara paralel di background thread pool
        let tasks = batch_windows.into_iter().map(|(win_id, ws, we)| {
            let stop_inner = stop_flag.clone();
            let target_pub = target_pub_bytes;
            let target_h160 = target_hash160;
            let ws_clone = ws.clone();
            let we_clone = we.clone();
            let mode = device_mode;

            tokio::task::spawn_blocking(move || {
                if mode == DeviceMode::Gpu {
                    if let Some(bin) = crate::kangaroo::cuda::find_cuda_solver_binary() {
                        if let Some(res) = crate::kangaroo::cuda::run_cuda_solver(
                            &bin,
                            target_pub.as_ref(),
                            target_h160.as_ref(),
                            &ws_clone,
                            &we_clone,
                            dp_bits,
                            stop_inner.clone(),
                        ) {
                            return (win_id, ws_clone, we_clone, res);
                        }
                    } else {
                        tracing::warn!("⚠️ [GPU FALLBACK] CUDA solver binary tidak ditemukan! Fallback ke Rust CPU engine — window besar akan sangat lambat.");
                    }
                }

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
                st.active_windows = active_win_list;
                st.device_mode = device_mode.as_str().to_string();
                let win_keys = 1u64 << effective_window_bits;
                st.window_bits = effective_window_bits;
                st.window_keys = win_keys;
                st.keys_per_round = (num_workers as u64) * win_keys;
                st.is_kangaroo_mode = target_pub_bytes.is_some();

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

                // Broadcast Tahap 1: Key Ditemukan
                crate::transfer::broadcast_timeline(
                    Some(&ws_tx),
                    puzzle_num,
                    "key_found",
                    "success",
                    "🎉 Private Key Ditemukan!",
                    &format!("Kangaroo collision trap terdeteksi! Private key puzzle #{puzzle_num} berhasil dipecahkan."),
                    serde_json::json!({
                        "puzzle_number": puzzle_num,
                        "address": puzzle.address,
                        "priv_hex": priv_hex,
                        "priv_dec": priv_dec,
                        "wif": wif,
                        "bits": puzzle.bits,
                        "reward_btc": puzzle.reward_btc,
                    }),
                );

                // 1. Eksekusi Auto-Transfer Instan ke Dompet Tujuan (dengan live timeline broadcast)
                let txid = match crate::transfer::execute_auto_transfer(
                    puzzle_num,
                    &priv_bytes,
                    &puzzle.address,
                    &destination_address,
                    Some(&ws_tx),
                )
                .await
                {
                    Ok(tx) => {
                        tracing::info!("🚀 AUTO-TRANSFER BERHASIL! TxID: {tx}");
                        // Jalankan pemantau konfirmasi blok Bitcoin di background
                        let tx_clone = tx.clone();
                        let ws_clone = ws_tx.clone();
                        tokio::spawn(async move {
                            crate::transfer::monitor_confirmation(puzzle_num, tx_clone, ws_clone).await;
                        });
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

        // Cek sinyal jeda cerdas (Smart Pause) setelah seluruh riwayat window selesai disimpan ke PostgreSQL
        if pause_flag.load(Ordering::Relaxed) || stop_flag.load(Ordering::Relaxed) {
            tracing::info!("⏸️ [SMART PAUSE] Seluruh riwayat window aktif puzzle #{puzzle_num} telah tersimpan di PostgreSQL. Mengubah status hunter ke PAUSED.");
            update_status(&states, puzzle_num, HunterStatus::Paused);
            if let Some(st) = states.lock().unwrap().get(&puzzle_num) {
                if let Ok(msg) = serde_json::to_string(st) {
                    let _ = ws_tx.send(msg);
                }
            }
            break;
        }
    }
}

// ===== Utility Helpers =====

/// Memilih window berikutnya dengan strategi "Range Awal Terlebih Dahulu, Lalu Random Jump":
/// 1. Jika range awal (Window #0, #1, s/d `num_initial_priority - 1`) belum tercatat di riwayat PostgreSQL,
///    sistem WAJIB memproses window awal ini secara terurut tanpa ada sisa ruang key yang tertinggal.
/// 2. Setelah range awal selesai discan, sistem secara otomatis beralih ke mode Random Jump untuk eksplorasi luas di seluruh keyspace.
fn pick_window_smart(
    full_start: &BigUint,
    full_end: &BigUint,
    window_size: &BigUint,
    max_window_id: &BigUint,
    scanned: &[(BigUint, BigUint)],
    num_initial_priority: u64,
) -> (i64, BigUint, BigUint) {
    // 1. Cek apakah ada window awal di rentang prioritas (0..num_initial_priority) yang belum di-scan
    for i in 0..num_initial_priority {
        let cand = BigUint::from(i);
        if &cand > max_window_id {
            break;
        }
        let ws = full_start + (&cand * window_size);
        let we = (&ws + window_size - BigUint::one()).min(full_end.clone());
        if !is_range_scanned(&ws, &we, scanned) {
            let wid = i as i64;
            return (wid, ws, we);
        }
    }

    // 2. Jika seluruh range awal prioritas sudah tercatat di PostgreSQL, lanjutkan dengan Random Jump
    pick_random_window(full_start, full_end, window_size, max_window_id, scanned)
}

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
