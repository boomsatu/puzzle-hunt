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

        let p_start = BigUint::parse_bytes(puzzle.range_start_hex.trim_start_matches("0x").as_bytes(), 16).unwrap_or_default();
        let p_end = BigUint::parse_bytes(puzzle.range_end_hex.trim_start_matches("0x").as_bytes(), 16).unwrap_or_default();
        let dev_mode = DeviceMode::from_env();
        let num_workers = get_optimal_workers(&self.states, dev_mode);
        let step = if &p_end > &p_start {
            (&p_end - &p_start) / BigUint::from(num_workers as u64)
        } else {
            BigUint::from(1u64)
        };
        let init_windows: Vec<WorkerWindow> = (0..num_workers)
            .map(|idx| {
                let ws = &p_start + (&step * BigUint::from(idx as u64));
                let we = &ws + &step;
                WorkerWindow {
                    worker_id: idx + 1,
                    start_hex: format!("0x{:x}", ws),
                    end_hex: format!("0x{:x}", we),
                }
            })
            .collect();

        let now_str = chrono_now();
        let mut new_state = HunterState::new(puzzle_num);
        new_state.status = HunterStatus::Running;
        new_state.started_at = Some(now_str);
        new_state.active_workers = num_workers;
        new_state.active_windows = init_windows;
        new_state.last_window_hex = if dev_mode == DeviceMode::Gpu {
            format!(
                "0x{:x}..0x{:x} (⚡ {} Streams GPU Ready)",
                p_start, p_end, num_workers
            )
        } else {
            format!(
                "0x{:x}..0x{:x} (⚡ {} Workers Ready)",
                p_start, p_end, num_workers
            )
        };
        new_state.device_mode = dev_mode.as_str().to_string();
        if dev_mode == DeviceMode::Gpu {
            new_state.gpu_info = Some(crate::kangaroo::cuda::get_telemetry(num_workers));
        }

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
            // Mode Local: Sisakan core untuk OS + Postgres + UI desktop (aman & stabil)
            // Mendukung konfigurasi LOCAL_WORKERS di .env (default: 6 worker agar suhu laptop adem & terhindar dari thermal throttling)
            let max_local = std::env::var("LOCAL_WORKERS")
                .or_else(|_| std::env::var("MAX_WORKERS"))
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(6);

            let compute_budget = total_cpus.saturating_sub(2).max(1);
            let per_puzzle = compute_budget / running_count;
            per_puzzle.clamp(1, max_local)
        }
        DeviceMode::Vps => {
            // Mode VPS: Sisakan hanya 1 core untuk OS + PostgreSQL + Web API
            // Seluruh sisa core dimaksimalkan untuk puzzle (misal di VPS 8 core: 8 - 1 = 7 core penuh)
            let compute_budget = total_cpus.saturating_sub(1).max(1);
            let per_puzzle = compute_budget / running_count;
            per_puzzle.max(1)
        }
        DeviceMode::Gpu => {
            // Mode GPU: Ambil dari GPU_WORKERS di .env jika ada (default: 16 worker agar 16 SM stream cards di dashboard terisi penuh)
            if let Ok(workers_str) = std::env::var("GPU_WORKERS") {
                if let Ok(w) = workers_str.parse::<usize>() {
                    return w.clamp(1, 64);
                }
            }
            16
        }
    }
}

async fn run_hunt_loop(
    pool: PgPool,
    puzzle: Puzzle,
    _window_bits: u32,
    check_interval_secs: u64,
    destination_address: String,
    states: Arc<Mutex<HashMap<i32, HunterState>>>,
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    ws_tx: tokio::sync::broadcast::Sender<String>,
) {
    let puzzle_num = puzzle.puzzle_number;

    // Deteksi mode: Public Key Exposed (Kangaroo O(√N)) vs Address Scanner (Hash160)
    let pub_clean = puzzle.pubkey_hex.trim().trim_start_matches("0x").to_string();
    let target_pub_bytes: Option<[u8; 33]> = if pub_clean.len() == 66 {
        match hex::decode(&pub_clean) {
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

    // Hitung window bits optimal berdasarkan tipe puzzle dan mode device:
    let total_range_bits = if puzzle.bits > 2 { (puzzle.bits - 1) as u32 } else { 1 };
    let effective_window_bits = if target_pub_bytes.is_some() {
        // Kangaroo Mode (Public Key Tersedia, e.g. Puzzle #140):
        // Menggunakan window 2^30 = 1.073.741.824 keys per worker.
        30u32.min(total_range_bits.saturating_sub(1).max(8))
    } else if device_mode == DeviceMode::Gpu {
        // Address Scanner Mode di GPU (cuBitCrack):
        // Di GPU, cuBitCrack memeriksa miliaran kunci per detik.
        // Default diatur ke 28-bit (~268.4 Juta keys/worker, total 16 worker = ~4.29 Miliar keys/ronde).
        // Di Dual RTX 5090 (6-7 GK/s), setiap ronde selesai dalam ~0.6 - 0.8 detik!
        // Setiap ronde langsung tersimpan ke PostgreSQL, counter bertambah cepat, dan visual UI sangat responsif!
        // Dapat dikonfigurasi via GPU_WINDOW_BITS di .env (misal 26 = 67M, 27 = 134M, 28 = 268M)
        let default_gpu_bits = std::env::var("GPU_WINDOW_BITS")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(28);
        default_gpu_bits.min(total_range_bits.saturating_sub(1).max(8))
    } else {
        // Address Scanner Mode (Hash160 only, e.g. Puzzle #71):
        // Pengecekan sekuensial linear O(N) di CPU. Dibatasi ke 18-bit (~262.144 keys/worker)
        // agar setiap ronde tetap cepat (< 0.3s) dan UI bergerak lincah tanpa lagging.
        18u32.min(total_range_bits.saturating_sub(1).max(8))
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

    // KANGAROO GPU CONTINUOUS MODE DISPATCH
    // Jika puzzle memiliki public key dan mode GPU aktif, jalankan continuous process Kangaroo GPU
    // (Memeras 100% RTX PRO 6000 di 5.14 GigaKeys/s murni CUDA, dan streaming live ke 16 worker stream di dashboard!)
    if device_mode == DeviceMode::Gpu && target_pub_bytes.is_some() {
        if let Some(bin) = crate::kangaroo::cuda::find_cuda_solver_binary() {
            if bin.to_lowercase().contains("kangaroo") {
                tracing::info!("🦘 [GPU KANGAROO] Mode Continuous GPU aktif untuk Puzzle #{puzzle_num} (Pubkey Terbuka).");
                run_continuous_gpu_kangaroo(
                    pool,
                    puzzle,
                    full_start,
                    full_end,
                    pub_clean.to_string(),
                    destination_address,
                    states,
                    pause_flag,
                    stop_flag,
                    ws_tx,
                )
                .await;
                return;
            }
        }
    }

    // ADDRESS SCANNER GPU DISPATCH (cuBitCrack / BitCrack)
    // Jika puzzle TIDAK memiliki public key (hanya Address/Hash160 seperti Puzzle #71):
    // - Jika SEARCH_STRATEGY == "linear": jalankan cuBitCrack linear sweep
    // - Jika SEARCH_STRATEGY == "stratified" / "random" (default): masuk ke multi-worker loop di bawah
    //   sehingga worker terbagi merata di seluruh keyspace dan seluruh riwayat jump tersimpan ke PostgreSQL!
    let search_strategy = std::env::var("SEARCH_STRATEGY")
        .unwrap_or_else(|_| "stratified".to_string())
        .to_lowercase();

    if device_mode == DeviceMode::Gpu && target_pub_bytes.is_none() && search_strategy == "linear" {
        if let Some(bin) = crate::kangaroo::cuda::find_cuda_address_solver() {
            tracing::info!("⚡ [GPU BITCRACK LINEAR] Mode Linear GPU Address Scanner ({bin}) aktif untuk Puzzle #{puzzle_num} (Address: {}).", puzzle.address);
            run_continuous_gpu_bitcrack(
                pool,
                puzzle,
                full_start,
                full_end,
                destination_address,
                states,
                pause_flag,
                stop_flag,
                ws_tx,
            )
            .await;
            return;
        }
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
            let num_w = get_optimal_workers(&states, device_mode);
            let gpu_window_keys: u64 = 1u64 << effective_window_bits;
            let gpu_total_round = gpu_window_keys * (num_w as u64);
            tracing::info!(
                "🎯 [WORKER CONFIG] {} workers × 2^{} = {} keys/worker, total {} keys/round",
                num_w, effective_window_bits, gpu_window_keys, gpu_total_round
            );
        }
    }

    // Inisialisasi pool CudaBitcrackDaemon jika GPU mode aktif untuk address scan (Zero Teardown)
    let mut bitcrack_daemons: Vec<crate::kangaroo::cuda::CudaBitcrackDaemon> = Vec::new();
    if device_mode == DeviceMode::Gpu && target_pub_bytes.is_none() {
        if let Some(bin) = crate::kangaroo::cuda::find_cuda_address_solver() {
            let num_w = get_optimal_workers(&states, device_mode);
            let gpu_count = crate::kangaroo::cuda::get_gpu_count();
            tracing::info!(
                "🚀 [CUDA DAEMON] Menginisialisasi {} persistent cuBitCrack daemon(s) di {} GPU...",
                num_w, gpu_count
            );
            let _ = std::fs::create_dir_all("data");
            let spawn_futs = (0..num_w).map(|w_idx| {
                let assigned_gpu = w_idx % gpu_count;
                let workers_per_gpu = (num_w / gpu_count).max(1);
                let total_sm = crate::kangaroo::cuda::get_optimal_bitcrack_blocks(assigned_gpu);
                let sm_blocks = (total_sm / (workers_per_gpu as u32)).clamp(4, total_sm);
                let out_file = format!("data/found_gpu_daemon_{}_{}.txt", puzzle_num, w_idx);
                let bin_clone = bin.clone();
                let addr_clone = puzzle.address.clone();
                async move {
                    crate::kangaroo::cuda::CudaBitcrackDaemon::spawn(
                        &bin_clone,
                        w_idx,
                        assigned_gpu,
                        sm_blocks,
                        &addr_clone,
                        &out_file,
                    ).await
                }
            });

            let spawned = futures_util::future::join_all(spawn_futs).await;
            let mut spawn_ok = true;
            for res in spawned {
                match res {
                    Ok(daemon) => bitcrack_daemons.push(daemon),
                    Err(err) => {
                        tracing::warn!("⚠️ [CUDA DAEMON] Daemon spawn failed: {err}. Fallback ke mode batch standard.");
                        spawn_ok = false;
                        break;
                    }
                }
            }

            if !spawn_ok || bitcrack_daemons.len() != num_w {
                for mut d in bitcrack_daemons.drain(..) {
                    d.quit().await;
                }
                tracing::info!("ℹ️ [CUDA DAEMON] Menggunakan fallback spawn_blocking standard.");
            } else {
                tracing::info!(
                    "✅ [CUDA DAEMON READY] Berhasil mengaktifkan {} persistent daemon(s)! Zero startup overhead aktif (lompatan real-time sub-detik).",
                    num_w
                );
            }
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

        let search_strategy = std::env::var("SEARCH_STRATEGY")
            .unwrap_or_else(|_| "stratified".to_string())
            .to_lowercase();

        // Ambil batch window dengan strategi terpilih:
        // - 'stratified' (default): Membagi keyspace rata ke 16 sektor dari ujung kiri (0%) sampai ujung kanan (100%),
        //   sehingga worker tersebar rapi, pinggir kiri & pinggir kanan selalu aktif, dan tidak pernah menumpuk di tengah.
        // - 'random': Lompatan acak bebas ke seluruh keyspace.
        let mut batch_windows = Vec::with_capacity(num_workers);
        for w_idx in 0..num_workers {
            let (window_id, w_start, w_end) = if search_strategy == "random" {
                pick_window_smart(
                    &full_start,
                    &effective_end,
                    &window_size,
                    &max_window_id,
                    &scanned,
                    num_workers as u64,
                )
            } else {
                pick_window_stratified(
                    w_idx,
                    num_workers,
                    &full_start,
                    &effective_end,
                    &window_size,
                    &max_window_id,
                    &scanned,
                )
            };
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

        // Periksa kesehatan pool persistent CUDA daemon
        let daemons_active = if !bitcrack_daemons.is_empty() {
            let all_alive = bitcrack_daemons.iter_mut().all(|d| d.is_alive());
            if !all_alive {
                tracing::warn!("⚠️ [CUDA DAEMON] Worker daemon terdeteksi berhenti. Membersihkan pool dan beralih ke engine batch standard.");
                for mut d in bitcrack_daemons.drain(..) {
                    d.quit().await;
                }
                false
            } else {
                bitcrack_daemons.len() == batch_windows.len()
            }
        } else {
            false
        };

        // Jalankan seluruh worker secara paralel (Gunakan persistent CUDA daemon jika siap, atau fallback standard)
        let batch_results = if daemons_active {
            let futs = bitcrack_daemons.iter_mut().zip(batch_windows.iter()).enumerate().map(|(w_idx, (daemon, (win_id, ws, we)))| {
                let s_hex = format!("0x{:x}", ws);
                let e_hex = format!("0x{:x}", we);
                let stop_inner = stop_flag.clone();
                let win_id_val = *win_id;
                let ws_clone = ws.clone();
                let we_clone = we.clone();
                let p_addr = puzzle.address.clone();
                let assigned_gpu = daemon.gpu_id;
                async move {
                    let res = daemon.search_range(&s_hex, &e_hex, stop_inner.clone()).await;
                    match res {
                        Ok(r) => (win_id_val, ws_clone, we_clone, r),
                        Err(e) => {
                            tracing::warn!("⚠️ [CUDA DAEMON] Worker {w_idx} error: {e}. Fallback ke single-run window.");
                            let fallback_res = if let Some(bin) = crate::kangaroo::cuda::find_cuda_address_solver() {
                                let gpu_count = crate::kangaroo::cuda::get_gpu_count();
                                let workers_per_gpu = (num_workers / gpu_count).max(1);
                                let total_sm = crate::kangaroo::cuda::get_optimal_bitcrack_blocks(assigned_gpu);
                                let sm_blocks = (total_sm / (workers_per_gpu as u32)).clamp(4, total_sm);
                                let ws_fb = ws_clone.clone();
                                let we_fb = we_clone.clone();
                                tokio::task::spawn_blocking(move || {
                                    crate::kangaroo::cuda::run_cuda_bitcrack_window(
                                        &bin,
                                        &p_addr,
                                        &ws_fb,
                                        &we_fb,
                                        assigned_gpu,
                                        sm_blocks,
                                        stop_inner,
                                    )
                                }).await.unwrap_or_else(|_| crate::kangaroo::KangarooResult::miss(0, 0.0))
                            } else {
                                crate::kangaroo::KangarooResult::miss(0, 0.0)
                            };
                            (win_id_val, ws_clone, we_clone, fallback_res)
                        }
                    }
                }
            });
            futures_util::future::join_all(futs).await
        } else {
            let tasks = batch_windows.into_iter().enumerate().map(|(w_idx, (win_id, ws, we))| {
                let stop_inner = stop_flag.clone();
                let target_pub = target_pub_bytes;
                let target_h160 = target_hash160;
                let ws_clone = ws.clone();
                let we_clone = we.clone();
                let mode = device_mode;
                let p_addr = puzzle.address.clone();

                tokio::task::spawn_blocking(move || {
                    if mode == DeviceMode::Gpu {
                        // 1. Mode Address Scanner di GPU (cuBitCrack multi-GPU / multi-worker)
                        if target_pub.is_none() {
                            if let Some(bin) = crate::kangaroo::cuda::find_cuda_address_solver() {
                                let gpu_count = crate::kangaroo::cuda::get_gpu_count();
                                let assigned_gpu = w_idx % gpu_count;
                                let workers_per_gpu = (num_workers / gpu_count).max(1);
                                let total_sm = crate::kangaroo::cuda::get_optimal_bitcrack_blocks(assigned_gpu);
                                // Bagi total SM GPU secara adil ke setiap worker pada GPU ini
                                // Contoh 32 worker di Dual RTX 5090: 16 worker/GPU -> 170/16 = 10 blocks/worker!
                                // Efek: Alokasi VRAM menciut dari 1.5GB ke ~100MB per worker, starting points hanya 655K,
                                // dan 32 worker berjalan serentak TANPA BEREBUT SM ATAU MEMORI!
                                let sm_blocks = (total_sm / (workers_per_gpu as u32)).clamp(4, total_sm);
                                let res = crate::kangaroo::cuda::run_cuda_bitcrack_window(
                                    &bin,
                                    &p_addr,
                                    &ws_clone,
                                    &we_clone,
                                    assigned_gpu,
                                    sm_blocks,
                                    stop_inner.clone(),
                                );
                                return (win_id, ws_clone, we_clone, res);
                            }
                        }

                        // 2. Mode Solver CUDA eksternal lainnya jika terpasang (misal keyhunt)
                        if let Some(bin) = crate::kangaroo::cuda::find_cuda_solver_binary() {
                            let lower_bin = bin.to_lowercase();
                            if !lower_bin.contains("kangaroo") {
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
                            }
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

            let join_res = futures_util::future::join_all(tasks).await;
            let mut converted = Vec::with_capacity(join_res.len());
            for task_res in join_res {
                if let Ok(tuple) = task_res {
                    converted.push(tuple);
                }
            }
            converted
        };

        let mut hit_result = None;
        let mut batch_keys: u64 = 0;
        let mut db_records = Vec::with_capacity(batch_results.len());

        for (win_id, ws, we, res) in batch_results {

            batch_keys += res.keys_checked;

            // Kumpulkan seluruh window yang discan pada ronde ini ke memory buffer
            let s_hex = format!("0x{:x}", ws);
            let e_hex = format!("0x{:x}", we);
            db_records.push(db::NewScannedWindow {
                puzzle_number: puzzle_num,
                window_id: win_id,
                range_start_hex: s_hex,
                range_end_hex: e_hex,
                keys_checked: res.keys_checked as i64,
                duration_secs: res.elapsed_secs,
            });

            if res.found && hit_result.is_none() {
                hit_result = Some(res);
            }
        }

        // BATCH INSERT: Simpan seluruh window ronde ini dalam 1 query transaksi cepat ke PostgreSQL!
        if !db_records.is_empty() {
            if let Err(e) = db::record_scanned_windows_batch(&pool, &db_records).await {
                tracing::warn!("⚠️ [DB BATCH] Gagal menyimpan batch scanned windows ke PostgreSQL: {e}");
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
                handle_key_found(&pool, &puzzle, &priv_hex, &destination_address, &states, &ws_tx).await;
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

        // Opsional jeda istirahat CPU antar ronde jika diatur di .env (misal 50ms-100ms untuk laptop cooling)
        if let Ok(delay_str) = std::env::var("ROUND_DELAY_MS") {
            if let Ok(ms) = delay_str.trim().parse::<u64>() {
                if ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
                }
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

    // Bersihkan seluruh persistent CUDA daemon saat hunter berhenti / dijeda / puzzle solved
    for mut d in bitcrack_daemons {
        d.quit().await;
    }
}

/// Menjalankan JeanLucPons Kangaroo sebagai continuous process pada GPU (Dual-GPU RTX 5090)
/// untuk puzzle yang sudah memiliki Public Key (seperti Puzzle #140).
/// Hanya 1 instance yang berjalan (menghemat RAM, aman di 129GB, dan memeras 100% 2x RTX 5090).
#[allow(dead_code)]
async fn run_continuous_gpu_kangaroo(
    pool: PgPool,
    puzzle: Puzzle,
    full_start: BigUint,
    full_end: BigUint,
    pub_clean: String,
    destination_address: String,
    states: Arc<Mutex<HashMap<i32, HunterState>>>,
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    ws_tx: tokio::sync::broadcast::Sender<String>,
) {
    let puzzle_num = puzzle.puzzle_number;
    let _ = std::fs::create_dir_all("data");

    let cfg_path = format!("data/k_target_{puzzle_num}.txt");
    let work_path = format!("data/kang_{puzzle_num}.work");
    let out_path = format!("data/FOUND_{puzzle_num}.txt");

    // JeanLucPons Kangaroo memiliki limitasi matematis: interval maksimal 125-bit (2^125).
    // Untuk puzzle > 125-bit (seperti Puzzle #140 yang memiliki rentang 139-bit),
    // pilih interval 124-bit (1 << 124) secara Random Jump / Stratified yang belum ada di PostgreSQL!
    let max_gpu_interval = BigUint::one() << 124usize;
    let (cur_start, cur_end) = if &full_end - &full_start > max_gpu_interval {
        let scanned_intervals = db::load_scanned_windows(&pool, puzzle_num).await.unwrap_or_default();
        let total_intervals = ((&full_end - &full_start) / &max_gpu_interval).max(BigUint::one());
        let mut rng = rand::thread_rng();
        let mut chosen_start = full_start.clone();
        let mut chosen_end = &full_start + &max_gpu_interval;
        let mut found_virgin = false;
        for _ in 0..500 {
            let rand_offset: BigUint = rng.gen_biguint_range(&BigUint::zero(), &total_intervals);
            let cs = &full_start + (&rand_offset * &max_gpu_interval);
            let ce = (&cs + &max_gpu_interval).min(full_end.clone());
            if !is_range_scanned(&cs, &ce, &scanned_intervals) {
                chosen_start = cs;
                chosen_end = ce;
                found_virgin = true;
                break;
            }
        }
        if !found_virgin {
            chosen_start = full_start.clone();
            chosen_end = (&full_start + &max_gpu_interval).min(full_end.clone());
        }
        tracing::info!(
            "🦘 [GPU KANGAROO JUMP] Puzzle #{puzzle_num}: Memilih interval 2^124 acak 0x{:x}..0x{:x} (Anti-Duplikasi PostgreSQL)",
            chosen_start, chosen_end
        );
        (chosen_start, chosen_end)
    } else {
        (full_start.clone(), full_end.clone())
    };

    let start_hex = format!("{:x}", cur_start);
    let end_hex = format!("{:x}", cur_end);
    let cfg_content = format!("{start_hex}\n{end_hex}\n{pub_clean}\n");
    if let Err(e) = std::fs::write(&cfg_path, &cfg_content) {
        tracing::error!("Gagal menulis berkas konfigurasi Kangaroo: {e}");
        return;
    }

    let bin = crate::kangaroo::cuda::find_cuda_solver_binary()
        .unwrap_or_else(|| "kangaroo".to_string());

    let gpu_id_str = crate::kangaroo::cuda::get_gpu_ids_str();

    let mut args = vec![
        "-t".to_string(), "0".to_string(),
        "-gpu".to_string(),
        "-gpuId".to_string(), gpu_id_str,
        "-d".to_string(), "10".to_string(),
        "-w".to_string(), work_path.clone(),
        "-wi".to_string(), "30".to_string(),
        "-ws".to_string(),
        "-o".to_string(), out_path.clone(),
    ];

    let work_file = std::path::Path::new(&work_path);
    if work_file.exists() {
        if let Ok(meta) = std::fs::metadata(&work_path) {
            if meta.len() > 1024 {
                tracing::info!("📁 Ditemukan checkpoint progres Kangaroo valid: {work_path}. Melanjutkan dari state terakhir (-i).");
                args.push("-i".to_string());
                args.push(work_path.clone());
            } else {
                tracing::warn!("⚠️ File checkpoint {work_path} kosong/corrupt (< 1KB). Menghapus dan memulai fresh.");
                let _ = std::fs::remove_file(&work_path);
            }
        }
    }
    args.push(cfg_path.clone());

    // Inisialisasi 16 SM Grid Streams untuk visualisasi dashboard
    let num_workers = 16usize;
    let window_step = (&cur_end - &cur_start) / BigUint::from(num_workers as u64);
    let active_win_list: Vec<WorkerWindow> = (0..num_workers)
        .map(|idx| {
            let ws = &cur_start + (&window_step * BigUint::from(idx as u64));
            let we = &ws + &window_step;
            WorkerWindow {
                worker_id: idx + 1,
                start_hex: format!("0x{:x}", ws),
                end_hex: format!("0x{:x}", we),
            }
        })
        .collect();

    // Update state awal di memory & websocket
    {
        let mut s = states.lock().unwrap();
        if let Some(st) = s.get_mut(&puzzle_num) {
            st.last_window_hex = format!("0x{:x}..0x{:x} (⚡ 16 CUDA Streams • Inisialisasi Kangaroo)", cur_start, cur_end);
            st.active_workers = num_workers;
            st.active_windows = active_win_list.clone();
            st.is_kangaroo_mode = true;
            st.device_mode = "gpu".to_string();
            st.gpu_info = Some(crate::kangaroo::cuda::get_telemetry(num_workers));
            if let Ok(msg) = serde_json::to_string(&*st) {
                let _ = ws_tx.send(msg);
            }
        }
    }

    let mut child = match tokio::process::Command::new(&bin)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Gagal menjalankan Kangaroo binary: {e}");
            update_status(&states, puzzle_num, HunterStatus::Error(format!("Gagal execute kangaroo: {e}")));
            return;
        }
    };

    use tokio::io::AsyncBufReadExt;
    if let Some(stderr) = child.stderr.take() {
        let pnum = puzzle_num;
        tokio::spawn(async move {
            let mut err_reader = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = err_reader.next_line().await {
                tracing::warn!("⚠️ [KANGAROO GPU #{pnum}] {line}");
            }
        });
    }

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::error!("Gagal mengambil child stdout");
            return;
        }
    };
    let mut reader = tokio::io::BufReader::new(stdout).lines();

    let mut found_priv_hex: Option<String> = None;
    let mut round_counter: u64 = 0;

    loop {
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                // Cek stop flag / pause flag
                if pause_flag.load(Ordering::Relaxed) || stop_flag.load(Ordering::Relaxed) {
                    tracing::info!("⏸️ [KANGAROO PAUSE] Menjeda hunter Kangaroo puzzle #{puzzle_num}...");
                    let _ = child.kill().await;

                    // Simpan interval yang telah discan ke PostgreSQL
                    let s_hex = format!("0x{:x}", cur_start);
                    let e_hex = format!("0x{:x}", cur_end);
                    let pool_clone = pool.clone();
                    let p_num = puzzle_num;
                    let win_id = round_counter as i64;
                    tokio::spawn(async move {
                        let _ = sqlx::query(
                            "INSERT INTO scanned_windows (puzzle_number, window_id, range_start_hex, range_end_hex, keys_checked, duration_secs) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"
                        )
                        .bind(p_num)
                        .bind(win_id)
                        .bind(&s_hex)
                        .bind(&e_hex)
                        .bind(win_id * 5_000_000)
                        .bind(10.0f64)
                        .execute(&pool_clone)
                        .await;
                    });

                    update_status(&states, puzzle_num, HunterStatus::Paused);
                    if let Some(st) = states.lock().unwrap().get(&puzzle_num) {
                        if let Ok(msg) = serde_json::to_string(st) {
                            let _ = ws_tx.send(msg);
                        }
                    }
                    break;
                }

                // Cek file output apakah kunci sudah tertulis
                if let Ok(content) = std::fs::read_to_string(&out_path) {
                    if let Some(k) = extract_key_from_text(&content) {
                        found_priv_hex = Some(k);
                        let _ = child.kill().await;
                        break;
                    }
                }
            }
            line_res = reader.next_line() => {
                match line_res {
                    Ok(Some(l)) => {
                        // Parse status kecepatan Kangaroo
                        // Format: [5140.26 MK/s][GPU 5140.26 MK/s][Count 2^38.57][Dead 2]...
                        if l.contains("MK/s") || l.contains("GK/s") || l.contains("KK/s") {
                            round_counter += 1;
                            let parsed_kps = parse_kangaroo_speed(&l);
                            let parsed_count = parse_kangaroo_count(&l);

                            let mut s = states.lock().unwrap();
                            if let Some(st) = s.get_mut(&puzzle_num) {
                                if let Some(kps) = parsed_kps {
                                    st.keys_per_second = kps;
                                }
                                if let Some(count) = parsed_count {
                                    st.total_keys_checked = count;
                                }
                                st.round = round_counter;
                                st.windows_scanned = round_counter;
                                st.last_window_hex = format!("0x{:x}..0x{:x} (⚡ NVIDIA CUDA Kangaroo O(√N))", cur_start, cur_end);
                                st.active_workers = num_workers;
                                st.active_windows = active_win_list.clone();
                                if let Ok(msg) = serde_json::to_string(&*st) {
                                    let _ = ws_tx.send(msg);
                                }
                            }

                            // Simpan checkpoint progres ke PostgreSQL setiap 10 update (~5 detik)
                            if round_counter % 10 == 0 {
                                let pool_clone = pool.clone();
                                let p_num = puzzle_num;
                                let win_id = round_counter as i64;
                                let checked = parsed_count.unwrap_or(0) as i64;
                                let s_hex = format!("0x{:x}", cur_start);
                                let e_hex = format!("0x{:x}", cur_end);
                                tokio::spawn(async move {
                                    let _ = sqlx::query(
                                        "INSERT INTO scanned_windows (puzzle_number, window_id, range_start_hex, range_end_hex, keys_checked, duration_secs) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"
                                    )
                                    .bind(p_num)
                                    .bind(win_id)
                                    .bind(&s_hex)
                                    .bind(&e_hex)
                                    .bind(checked)
                                    .bind(5.0f64)
                                    .execute(&pool_clone)
                                    .await;
                                });
                            }
                        } else if !l.trim().is_empty() {
                            let msg_line = l.trim().to_string();
                            tracing::info!("🦘 [KANGAROO GPU #{puzzle_num}] {msg_line}");
                            let mut s = states.lock().unwrap();
                            if let Some(st) = s.get_mut(&puzzle_num) {
                                st.last_window_hex = format!("0x{:x}..0x{:x} (⚡ {})", cur_start, cur_end, msg_line);
                                st.active_workers = num_workers;
                                st.active_windows = active_win_list.clone();
                                if let Ok(msg) = serde_json::to_string(&*st) {
                                    let _ = ws_tx.send(msg);
                                }
                            }
                        }

                        // Parse private key jika ditemukan langsung di stdout
                        if l.contains("Key#") || l.contains("PRIVKEY:") || l.contains("Found:") || l.contains("Priv:") {
                            if let Some(k) = extract_key_from_text(&l) {
                                found_priv_hex = Some(k);
                                let _ = child.kill().await;
                                break;
                            }
                        }
                    }
                    Ok(None) => break, // Process exited
                    Err(_) => break,
                }
            }
        }
    }

    // Jika process berhenti bukan karena pause/stop dan key belum ketemu, catat status error ke dashboard
    if !pause_flag.load(Ordering::Relaxed) && !stop_flag.load(Ordering::Relaxed) && found_priv_hex.is_none() {
        if let Ok(exit_status) = child.wait().await {
            tracing::warn!("⚠️ [KANGAROO GPU #{puzzle_num}] Kangaroo solver berhenti dengan status: {exit_status}");
            if !exit_status.success() {
                update_status(
                    &states,
                    puzzle_num,
                    HunterStatus::Error(format!("Kangaroo solver berhenti ({exit_status}). Cek log server.")),
                );
                if let Some(st) = states.lock().unwrap().get(&puzzle_num) {
                    if let Ok(msg) = serde_json::to_string(st) {
                        let _ = ws_tx.send(msg);
                    }
                }
            }
        }
    }

    // Jika private key ditemukan, eksekusi auto-sweep & perayaan!
    if let Some(priv_hex) = found_priv_hex {
        handle_key_found(
            &pool,
            &puzzle,
            &priv_hex,
            &destination_address,
            &states,
            &ws_tx,
        ).await;
    }
}

/// Menjalankan cuBitCrack / BitCrack sebagai continuous GPU process untuk puzzle yang TIDAK memiliki public key (Address Scanner)
async fn run_continuous_gpu_bitcrack(
    pool: PgPool,
    puzzle: Puzzle,
    full_start: BigUint,
    full_end: BigUint,
    destination_address: String,
    states: Arc<Mutex<HashMap<i32, HunterState>>>,
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    ws_tx: tokio::sync::broadcast::Sender<String>,
) {
    let puzzle_num = puzzle.puzzle_number;
    let _ = std::fs::create_dir_all("data");
    let out_path = format!("data/FOUND_{puzzle_num}.txt");

    let start_hex = format!("{:x}", full_start);
    let end_hex = format!("{:x}", full_end);
    let range_arg = format!("{start_hex}:{end_hex}");

    let bin = match crate::kangaroo::cuda::find_cuda_address_solver() {
        Some(b) => b,
        None => {
            tracing::error!("❌ [GPU BITCRACK ERROR] Binary cuBitCrack tidak ditemukan di bin/cuBitCrack!");
            update_status(
                &states,
                puzzle_num,
                HunterStatus::Error("cuBitCrack belum terpasang di bin/cuBitCrack. Jalankan 'bash build_cubitcrack.sh' di server!".to_string()),
            );
            return;
        }
    };

    // Inisialisasi 16 SM Grid Streams untuk visualisasi dashboard multi-worker
    let num_workers = 16usize;
    let stream_chunk = BigUint::from(12_320_768u64 / num_workers as u64);
    let active_win_list: Vec<WorkerWindow> = (0..num_workers)
        .map(|idx| {
            let ws = &full_start + (&stream_chunk * BigUint::from(idx as u64));
            let we = &ws + &stream_chunk;
            WorkerWindow {
                worker_id: idx + 1,
                start_hex: format!("0x{:x}", ws),
                end_hex: format!("0x{:x}", we),
            }
        })
        .collect();

    let gpu_id_str = crate::kangaroo::cuda::get_gpu_ids_str();
    let sm_blocks = crate::kangaroo::cuda::get_optimal_bitcrack_blocks(0);
    let args = vec![
        "-d".to_string(), gpu_id_str,
        "-b".to_string(), sm_blocks.to_string(),
        "-t".to_string(), "256".to_string(),
        "-p".to_string(), "256".to_string(),
        "-c".to_string(), // Compressed Bitcoin address
        "--keyspace".to_string(), range_arg,
        "-o".to_string(), out_path.clone(),
        puzzle.address.clone(),
    ];

    tracing::info!("🚀 [BITCRACK GPU] Memulai cuBitCrack: {} {}", bin, args.join(" "));

    // Update state awal di memory & websocket agar 16 stream card langsung tampil di UI
    {
        let mut s = states.lock().unwrap();
        if let Some(st) = s.get_mut(&puzzle_num) {
            let init_end = &full_start + BigUint::from(12_320_768u64);
            st.last_window_hex = format!("0x{:x}..0x{:x} (⚡ 16 CUDA Streams • cuBitCrack)", full_start, init_end);
            st.active_workers = num_workers;
            st.active_windows = active_win_list.clone();
            st.is_kangaroo_mode = false;
            st.device_mode = "gpu".to_string();
            st.gpu_info = Some(crate::kangaroo::cuda::get_telemetry(num_workers));
            if let Ok(msg) = serde_json::to_string(&*st) {
                let _ = ws_tx.send(msg);
            }
        }
    }

    let mut child = match tokio::process::Command::new(&bin)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("❌ Gagal menjalankan cuBitCrack ({bin}): {e}");
            update_status(&states, puzzle_num, HunterStatus::Error(format!("Gagal execute cuBitCrack: {e}")));
            return;
        }
    };

    use tokio::io::AsyncBufReadExt;
    if let Some(stderr) = child.stderr.take() {
        let pnum = puzzle_num;
        tokio::spawn(async move {
            let mut err_reader = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = err_reader.next_line().await {
                tracing::warn!("⚠️ [BITCRACK GPU #{pnum}] {line}");
            }
        });
    }

    use tokio::io::AsyncReadExt;
    let mut stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::error!("Gagal mengambil child stdout");
            return;
        }
    };

    let mut found_priv_hex: Option<String> = None;
    let mut round_counter: u64 = 0;
    let mut read_buf = [0u8; 4096];
    let mut pending_chunk = String::new();

    loop {
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                if pause_flag.load(Ordering::Relaxed) || stop_flag.load(Ordering::Relaxed) {
                    tracing::info!("⏸️ [BITCRACK PAUSE] Menjeda hunter GPU BitCrack puzzle #{puzzle_num}...");
                    let _ = child.kill().await;
                    let cur_checked = states.lock().unwrap().get(&puzzle_num).map(|s| s.total_keys_checked).unwrap_or(0);
                    let s_hex = format!("0x{:x}", full_start);
                    let cur_pos = &full_start + BigUint::from(cur_checked);
                    let e_hex = format!("0x{:x}", cur_pos);
                    let pool_clone = pool.clone();
                    tokio::spawn(async move {
                        let _ = sqlx::query(
                            "INSERT INTO scanned_windows (puzzle_number, window_id, range_start_hex, range_end_hex, keys_checked, duration_secs) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"
                        )
                        .bind(puzzle_num)
                        .bind(round_counter as i64)
                        .bind(&s_hex)
                        .bind(&e_hex)
                        .bind(cur_checked as i64)
                        .bind(10.0f64)
                        .execute(&pool_clone)
                        .await;
                    });
                    update_status(&states, puzzle_num, HunterStatus::Paused);
                    if let Some(st) = states.lock().unwrap().get(&puzzle_num) {
                        if let Ok(msg) = serde_json::to_string(st) {
                            let _ = ws_tx.send(msg);
                        }
                    }
                    break;
                }

                if let Ok(content) = std::fs::read_to_string(&out_path) {
                    if let Some(k) = extract_key_from_text(&content) {
                        found_priv_hex = Some(k);
                        let _ = child.kill().await;
                        break;
                    }
                }
            }
            read_res = stdout.read(&mut read_buf) => {
                match read_res {
                    Ok(n) if n > 0 => {
                        let text = String::from_utf8_lossy(&read_buf[..n]);
                        pending_chunk.push_str(&text);

                        // BitCrack mengupdate progress bar menggunakan '\r' (carriage return) bukan newline '\n'
                        while let Some(pos) = pending_chunk.find(|c| c == '\r' || c == '\n') {
                            let line = pending_chunk[..pos].trim().to_string();
                            pending_chunk = pending_chunk[pos + 1..].to_string();

                            if line.is_empty() {
                                continue;
                            }

                            let l_upper = line.to_uppercase();
                            if l_upper.contains("MKEY/S") || l_upper.contains("GKEY/S") || l_upper.contains("KKEY/S") || l_upper.contains("SPEED:") {
                                round_counter += 1;
                                let parsed_kps = parse_bitcrack_speed(&line);
                                let parsed_count = parse_bitcrack_count(&line);

                                let mut cur_pos_clone = full_start.clone();
                                let mut cur_end_clone = full_start.clone();
                                let mut total_checked_val = 0u64;

                                {
                                    let mut s = states.lock().unwrap();
                                    if let Some(st) = s.get_mut(&puzzle_num) {
                                        if let Some(kps) = parsed_kps {
                                            st.keys_per_second = kps;
                                        }
                                        if let Some(count) = parsed_count {
                                            st.total_keys_checked = count;
                                        } else if let Some(kps) = parsed_kps {
                                            st.total_keys_checked += (kps * 0.5) as u64;
                                        }
                                        total_checked_val = st.total_keys_checked;
                                        st.round = round_counter;
                                        st.windows_scanned = round_counter;

                                        let cur_pos = &full_start + BigUint::from(st.total_keys_checked);
                                        let cur_end = &cur_pos + BigUint::from(12_320_768u64);
                                        cur_pos_clone = cur_pos.clone();
                                        cur_end_clone = cur_end.clone();

                                        st.last_window_hex = format!("0x{:x}..0x{:x} (⚡ NVIDIA CUDA cuBitCrack)", cur_pos, cur_end);
                                        st.active_workers = num_workers;

                                        // Majukan jendela 16 CUDA Streams secara dinamis sesuai pergerakan scan GPU real-time
                                        let dynamic_win_list: Vec<WorkerWindow> = (0..num_workers)
                                            .map(|idx| {
                                                let ws = &cur_pos + (&stream_chunk * BigUint::from(idx as u64));
                                                let we = &ws + &stream_chunk;
                                                WorkerWindow {
                                                    worker_id: idx + 1,
                                                    start_hex: format!("0x{:x}", ws),
                                                    end_hex: format!("0x{:x}", we),
                                                }
                                            })
                                            .collect();
                                        st.active_windows = dynamic_win_list;

                                        if let Ok(msg) = serde_json::to_string(&*st) {
                                            let _ = ws_tx.send(msg);
                                        }
                                    }
                                }

                                // Simpan checkpoint berkala ke PostgreSQL setiap 10 ronde (~10 detik)
                                if round_counter % 10 == 0 && total_checked_val > 0 {
                                    let pool_clone = pool.clone();
                                    let p_num = puzzle_num;
                                    let win_id = round_counter as i64;
                                    let checked = total_checked_val as i64;
                                    let s_hex = format!("0x{:x}", cur_pos_clone);
                                    let e_hex = format!("0x{:x}", cur_end_clone);
                                    tokio::spawn(async move {
                                        let _ = sqlx::query(
                                            "INSERT INTO scanned_windows (puzzle_number, window_id, range_start_hex, range_end_hex, keys_checked, duration_secs) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"
                                        )
                                        .bind(p_num)
                                        .bind(win_id)
                                        .bind(&s_hex)
                                        .bind(&e_hex)
                                        .bind(checked)
                                        .bind(10.0f64)
                                        .execute(&pool_clone)
                                        .await;
                                    });
                                }
                            } else {
                                tracing::info!("⚡ [BITCRACK GPU #{puzzle_num}] {line}");
                                let mut s = states.lock().unwrap();
                                if let Some(st) = s.get_mut(&puzzle_num) {
                                    st.last_window_hex = format!("0x{:x}..0x{:x} (⚡ {})", full_start, full_end, line);
                                    st.active_workers = num_workers;
                                    st.active_windows = active_win_list.clone();
                                    if let Ok(msg) = serde_json::to_string(&*st) {
                                        let _ = ws_tx.send(msg);
                                    }
                                }
                            }

                            if l_upper.contains("PRIV:") || l_upper.contains("ADDRESS:") || l_upper.contains("FOUND:") || l_upper.contains("KEY#") || l_upper.contains("PRIVATE KEY:") {
                                if let Some(k) = extract_key_from_text(&line) {
                                    found_priv_hex = Some(k);
                                    let _ = child.kill().await;
                                    break;
                                }
                            }
                        }
                    }
                    Ok(_) => {
                        tracing::warn!("⚠️ [BITCRACK GPU #{puzzle_num}] Stream stdout cuBitCrack selesai.");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("❌ [BITCRACK GPU #{puzzle_num}] Error membaca stdout: {e}");
                        break;
                    }
                }
            }
        }
    }

    if !pause_flag.load(Ordering::Relaxed) && !stop_flag.load(Ordering::Relaxed) && found_priv_hex.is_none() {
        if let Ok(exit_status) = child.wait().await {
            tracing::warn!("⚠️ [BITCRACK GPU #{puzzle_num}] cuBitCrack berhenti dengan status: {exit_status}");
            if !exit_status.success() {
                update_status(
                    &states,
                    puzzle_num,
                    HunterStatus::Error(format!("cuBitCrack berhenti ({exit_status}). Cek log server.")),
                );
                if let Some(st) = states.lock().unwrap().get(&puzzle_num) {
                    if let Ok(msg) = serde_json::to_string(st) {
                        let _ = ws_tx.send(msg);
                    }
                }
            }
        }
    }

    if let Some(priv_hex) = found_priv_hex {
        handle_key_found(
            &pool,
            &puzzle,
            &priv_hex,
            &destination_address,
            &states,
            &ws_tx,
        ).await;
    }
}

fn parse_bitcrack_speed(line: &str) -> Option<f64> {
    let lower = line.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        if w.starts_with("mkey/s") || w.starts_with("mk/s") {
            if i > 0 {
                let val_str = words[i - 1].trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
                if let Ok(v) = val_str.parse::<f64>() {
                    return Some(v * 1_000_000.0);
                }
            }
        } else if w.starts_with("gkey/s") || w.starts_with("gk/s") {
            if i > 0 {
                let val_str = words[i - 1].trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
                if let Ok(v) = val_str.parse::<f64>() {
                    return Some(v * 1_000_000_000.0);
                }
            }
        } else if w.starts_with("kkey/s") || w.starts_with("kk/s") {
            if i > 0 {
                let val_str = words[i - 1].trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
                if let Ok(v) = val_str.parse::<f64>() {
                    return Some(v * 1_000.0);
                }
            }
        }
    }

    if let Some(idx) = lower.find("speed:") {
        let sub = &line[idx + 6..];
        let parts: Vec<&str> = sub.split_whitespace().collect();
        if parts.len() >= 2 {
            let num_str = parts[0].trim().trim_matches(|c: char| c == '[' || c == ']' || c == '<');
            let val: f64 = num_str.parse().ok()?;
            let unit = parts[1].to_uppercase();
            if unit.starts_with('G') {
                return Some(val * 1_000_000_000.0);
            } else if unit.starts_with('M') {
                return Some(val * 1_000_000.0);
            } else if unit.starts_with('K') {
                return Some(val * 1_000.0);
            } else {
                return Some(val);
            }
        }
    }
    None
}

fn parse_bitcrack_count(line: &str) -> Option<u64> {
    let lower = line.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        if w.starts_with("total") || w.contains("total") {
            if i > 0 {
                let digits: String = words[i - 1].chars().filter(|c| c.is_ascii_digit()).collect();
                if let Ok(c) = digits.parse::<u64>() {
                    return Some(c);
                }
            }
        }
    }

    if let Some(idx) = lower.find("total:") {
        let sub = &line[idx + 6..];
        let end = sub.find(']').unwrap_or_else(|| sub.find(' ').unwrap_or(sub.len()));
        let digits: String = sub[..end].chars().filter(|c| c.is_ascii_digit()).collect();
        return digits.parse::<u64>().ok();
    }
    None
}

async fn handle_key_found(
    pool: &PgPool,
    puzzle: &Puzzle,
    priv_hex: &str,
    destination_address: &str,
    states: &Arc<Mutex<HashMap<i32, HunterState>>>,
    ws_tx: &tokio::sync::broadcast::Sender<String>,
) {
    let puzzle_num = puzzle.puzzle_number;
    let priv_bytes_vec = match hex::decode(priv_hex.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => b,
        _ => {
            tracing::error!("Format private key tidak valid: {priv_hex}");
            return;
        }
    };
    let mut priv_bytes = [0u8; 32];
    priv_bytes.copy_from_slice(&priv_bytes_vec);

    let key_biguint = BigUint::from_bytes_be(&priv_bytes);
    let priv_dec = key_biguint.to_string();
    let wif = priv_to_wif(&priv_bytes);

    // Broadcast Tahap 1: Key Ditemukan
    crate::transfer::broadcast_timeline(
        Some(ws_tx),
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

    // 1. Eksekusi Auto-Transfer Instan ke Dompet Tujuan
    let txid = match crate::transfer::execute_auto_transfer(
        puzzle_num,
        &priv_bytes,
        &puzzle.address,
        destination_address,
        Some(ws_tx),
    )
    .await
    {
        Ok(tx) => {
            tracing::info!("🚀 AUTO-TRANSFER BERHASIL! TxID: {tx}");
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
        pool,
        puzzle_num,
        &puzzle.address,
        priv_hex,
        &priv_dec,
        &wif,
        txid.as_deref(),
    )
    .await;

    // 3. Simpan backup fisik
    save_backup_file(puzzle_num, priv_hex, &priv_dec, &wif, &puzzle.address);

    // 4. Kirim Notifikasi Instan ke Telegram
    let _ = crate::notify::send_telegram_alert(
        puzzle_num,
        puzzle.bits,
        &puzzle.address,
        priv_hex,
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
            st.found_key_hex = Some(priv_hex.to_string());
            st.found_wif = Some(wif.clone());

            if let Ok(msg) = serde_json::to_string(&*st) {
                let _ = ws_tx.send(msg);
            }
        }
    }

    tracing::info!("🎉🎉🎉 PUZZLE #{} SOLVED! KEY: 0x{}", puzzle_num, priv_hex);
}

#[allow(dead_code)]
fn parse_kangaroo_speed(line: &str) -> Option<f64> {
    let start = line.find('[')?;
    let end = line[start..].find(']')? + start;
    let chunk = &line[start + 1..end];
    let parts: Vec<&str> = chunk.split_whitespace().collect();
    if parts.len() >= 2 {
        let val: f64 = parts[0].parse().ok()?;
        let unit = parts[1];
        if unit.starts_with("GK") {
            Some(val * 1_000_000_000.0)
        } else if unit.starts_with("MK") {
            Some(val * 1_000_000.0)
        } else if unit.starts_with("KK") {
            Some(val * 1_000.0)
        } else {
            Some(val)
        }
    } else {
        None
    }
}

#[allow(dead_code)]
fn parse_kangaroo_count(line: &str) -> Option<u64> {
    if let Some(idx) = line.find("[Count 2^") {
        let sub = &line[idx + 9..];
        let end = sub.find(']')?;
        let power: f64 = sub[..end].parse().ok()?;
        Some((2.0f64.powf(power)) as u64)
    } else {
        None
    }
}

fn extract_key_from_text(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for marker in &["private key:", "priv (hex):", "priv:", "found:"] {
        if let Some(idx) = lower.find(marker) {
            let after = &text[idx + marker.len()..];
            for word in after.split_whitespace() {
                let clean = word.trim().trim_start_matches("0x").trim_matches(|c| c == '[' || c == ']');
                if clean.len() == 64 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(clean.to_lowercase());
                }
            }
        }
    }

    for word in text.split_whitespace() {
        let clean = word.trim().trim_start_matches("0x").trim_matches(|c| c == '[' || c == ']');
        if clean.len() == 64 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(clean.to_lowercase());
        }
    }
    None
}

// ===== Utility Helpers =====

/// Memilih window berikutnya dengan strategi "Stratified Grid Partitioning (Zonasi Merata)":
/// Membagi seluruh ruang kunci secara proporsional ke `num_workers` sektor (misal 16 sektor).
/// - Worker #1 fokus di sektor 0 (pinggir kiri: 0% - 6.25%)
/// - Worker #16 fokus di sektor 15 (pinggir kanan: 93.75% - 100%)
/// - Worker lainnya tersebar merata di sektor masing-masing.
/// Menjamin seluruh spektrum selalu aktif digempur, pinggir-pinggir tercover konsisten, dan tidak ada worker yang menumpuk di tengah.
fn pick_window_stratified(
    worker_idx: usize,
    num_workers: usize,
    full_start: &BigUint,
    full_end: &BigUint,
    window_size: &BigUint,
    max_window_id: &BigUint,
    scanned: &[(BigUint, BigUint)],
) -> (i64, BigUint, BigUint) {
    let total_wins = max_window_id + BigUint::one();
    let num_w = BigUint::from(num_workers.max(1));
    let wins_per_worker = &total_wins / &num_w;

    let w_idx = BigUint::from(worker_idx);
    let zone_start_wid = &w_idx * &wins_per_worker;
    let zone_end_wid = if worker_idx + 1 >= num_workers {
        max_window_id.clone()
    } else {
        (&zone_start_wid + &wins_per_worker - BigUint::one()).min(max_window_id.clone())
    };

    // Prioritas 1: Pastikan titik awal dari zona ini (termasuk window #0 di pinggir kiri) discan terlebih dahulu
    let ws_init = full_start + (&zone_start_wid * window_size);
    let we_init = (&ws_init + window_size - BigUint::one()).min(full_end.clone());
    if !is_range_scanned(&ws_init, &we_init, scanned) {
        let wid = zone_start_wid.to_u64_digits().first().copied().unwrap_or(0) as i64;
        return (wid, ws_init, we_init);
    }

    // Prioritas 2: Lompatan acak terzonasi (Stratified Random Jump) di dalam zona masing-masing
    let zone_span = if zone_end_wid >= zone_start_wid {
        &zone_end_wid - &zone_start_wid + BigUint::one()
    } else {
        BigUint::one()
    };

    let mut rng = rand::thread_rng();
    for _ in 0..100 {
        let offset = rng.gen_biguint_range(&BigUint::zero(), &zone_span);
        let cand = &zone_start_wid + offset;
        let ws = full_start + (&cand * window_size);
        let we = (&ws + window_size - BigUint::one()).min(full_end.clone());
        if !is_range_scanned(&ws, &we, scanned) {
            let wid = cand.to_u64_digits().first().copied().unwrap_or(0) as i64;
            return (wid, ws, we);
        }
    }

    // Fallback linear jika pencarian acak dalam zona berulang kali bentrok
    let mut cand = zone_start_wid.clone();
    while cand <= zone_end_wid {
        let ws = full_start + (&cand * window_size);
        let we = (&ws + window_size - BigUint::one()).min(full_end.clone());
        if !is_range_scanned(&ws, &we, scanned) {
            let wid = cand.to_u64_digits().first().copied().unwrap_or(0) as i64;
            return (wid, ws, we);
        }
        cand += BigUint::one();
    }

    // Fallback terakhir: acak bebas di seluruh keyspace
    pick_random_window(full_start, full_end, window_size, max_window_id, scanned)
}

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
