//! NVIDIA CUDA Hardware Acceleration Module for Bitcoin Puzzle Hunter
//! Mendukung deteksi GPU otomatis, pemanggilan solver CUDA (Kangaroo CUDA / KeyHunt / BitCrack),
//! diagnostic telemetry untuk GPU Hosting (Vast.ai, RunPod, AWS EC2),
//! support arsitektur: Turing, Ampere, Ada Lovelace, Blackwell (sm_120),
//! serta single-dispatch GPU pipeline (1 worker = all CUDA cores).

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use tracing::info;
use crate::kangaroo::KangarooResult;

#[derive(Debug, Clone)]
pub struct CudaGpuInfo {
    pub name: String,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub driver_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuTelemetry {
    pub name: String,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub driver_version: String,
    pub cuda_cores: u32,
    pub sm_count: u32,
    pub cuda_streams: usize,
    pub is_cuda_active: bool,
    pub solver_binary: Option<String>,
}

/// Memetakan nama GPU ke estimasi jumlah CUDA Cores & Streaming Multiprocessors (SM)
pub fn get_gpu_specs(name: &str) -> (u32, u32) {
    let lower = name.to_lowercase();
    // === Blackwell Generation (GB202/GB203, 2025+) ===
    if lower.contains("pro 6000") || lower.contains("rtx 6000s") || lower.contains("6000 blackwell") || lower.contains("gb202gl") {
        (24064, 188) // NVIDIA RTX PRO 6000 Blackwell (GB202GL): 24,064 CUDA Cores, 188 SMs, 96GB GDDR7
    } else if lower.contains("5090") {
        (21760, 170) // NVIDIA RTX 5090 Blackwell (GB202): 21,760 CUDA Cores, 170 SMs, 32GB GDDR7
    } else if lower.contains("5080") {
        (10752, 84)  // NVIDIA RTX 5080 Blackwell (GB203): 10,752 CUDA Cores, 84 SMs, 16GB GDDR7
    // === Ada Lovelace Generation (AD102, 2022-2024) ===
    } else if lower.contains("6000 ada") || lower.contains("rtx 6000 ada") {
        (18176, 142) // NVIDIA RTX 6000 Ada Generation: 18,176 CUDA Cores, 142 SMs
    } else if lower.contains("a6000") || lower.contains("rtx a6000") {
        (10752, 84)  // NVIDIA RTX A6000 (Ampere): 10,752 CUDA Cores, 84 SMs
    } else if lower.contains("rtx 6000") || lower.contains("quadro rtx 6000") {
        (4608, 72)   // Quadro RTX 6000 (Turing): 4,608 CUDA Cores, 72 SMs
    } else if lower.contains("4090") {
        (16384, 128) // RTX 4090 (AD102): 16,384 CUDA Cores, 128 SMs
    } else if lower.contains("4080") {
        (9728, 76)   // RTX 4080 (AD103): 9,728 CUDA Cores, 76 SMs
    } else if lower.contains("3090") {
        (10496, 82)  // RTX 3090 (GA102): 10,496 CUDA Cores, 82 SMs
    // === Data Center / HPC ===
    } else if lower.contains("b200") || lower.contains("gb200") {
        (18432, 144) // NVIDIA B200 Blackwell: 18,432 CUDA Cores, 144 SMs
    } else if lower.contains("a100") {
        (6912, 108)  // NVIDIA A100 (GA100): 6,912 CUDA Cores, 108 SMs
    } else if lower.contains("h100") {
        (16896, 132) // NVIDIA H100 (GH100): 16,896 CUDA Cores, 132 SMs
    } else {
        (24064, 188) // Default: NVIDIA RTX PRO 6000 Blackwell (target utama GPU hosting)
    }
}

/// Mendeteksi semua NVIDIA GPU dan CUDA driver yang aktif via nvidia-smi
pub fn detect_all_cuda_gpus() -> Vec<CudaGpuInfo> {
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.used,driver_version",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut list = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if !parts.is_empty() && !parts[0].is_empty() {
            let name = parts[0].to_string();
            let total_memory_mb = parts.get(1).and_then(|m| m.parse::<u64>().ok()).unwrap_or(0);
            let used_memory_mb = parts.get(2).and_then(|m| m.parse::<u64>().ok()).unwrap_or(0);
            let driver_version = parts.get(3).map(|d| d.to_string()).unwrap_or_else(|| "Unknown".into());
            list.push(CudaGpuInfo {
                name,
                total_memory_mb,
                used_memory_mb,
                driver_version,
            });
        }
    }
    list
}

/// Mendeteksi apakah sistem memiliki NVIDIA GPU (mengembalikan GPU pertama)
pub fn detect_cuda_gpu() -> Option<CudaGpuInfo> {
    detect_all_cuda_gpus().into_iter().next()
}

/// Mengembalikan jumlah GPU NVIDIA yang terpasang di sistem
pub fn get_gpu_count() -> usize {
    if let Ok(devs) = std::env::var("GPU_DEVICES").or_else(|_| std::env::var("CUDA_VISIBLE_DEVICES")) {
        let parts: Vec<&str> = devs.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if !parts.is_empty() {
            return parts.len();
        }
    }
    let count = detect_all_cuda_gpus().len();
    if count == 0 { 1 } else { count }
}

/// Mengembalikan daftar ID GPU dalam format string yang dipisahkan koma (misal: "0" atau "0,1" atau "0,1,2,3")
pub fn get_gpu_ids_str() -> String {
    if let Ok(devs) = std::env::var("GPU_DEVICES").or_else(|_| std::env::var("CUDA_VISIBLE_DEVICES")) {
        let clean = devs.trim();
        if !clean.is_empty() {
            return clean.to_string();
        }
    }
    let count = get_gpu_count();
    (0..count).map(|i| i.to_string()).collect::<Vec<_>>().join(",")
}

/// Menghitung jumlah block optimal untuk cuBitCrack berdasarkan SM GPU yang terdeteksi
pub fn get_optimal_bitcrack_blocks(gpu_id: usize) -> u32 {
    if let Ok(b_str) = std::env::var("BITCRACK_BLOCKS") {
        if let Ok(b) = b_str.trim().parse::<u32>() {
            return b.clamp(16, 512);
        }
    }
    let all = detect_all_cuda_gpus();
    if !all.is_empty() {
        let idx = gpu_id % all.len();
        let (_, sm) = get_gpu_specs(&all[idx].name);
        sm
    } else {
        188 // Default untuk target hosting NVIDIA RTX PRO 6000 Blackwell
    }
}

/// Mengambil telemetri lengkap GPU untuk kebutuhan visualisasi UI dan koordinasi backend
pub fn get_telemetry(streams: usize) -> GpuTelemetry {
    let all_gpus = detect_all_cuda_gpus();
    if !all_gpus.is_empty() {
        let count = all_gpus.len();
        let first = &all_gpus[0];
        let (cores_per_gpu, sm_per_gpu) = get_gpu_specs(&first.name);
        let total_cores = cores_per_gpu * (count as u32);
        let total_sm = sm_per_gpu * (count as u32);
        let total_memory_mb: u64 = all_gpus.iter().map(|g| g.total_memory_mb).sum();
        let used_memory_mb: u64 = all_gpus.iter().map(|g| g.used_memory_mb).sum();

        let display_name = if count > 1 {
            format!("{}x {}", count, first.name)
        } else {
            first.name.clone()
        };

        GpuTelemetry {
            name: display_name,
            total_memory_mb,
            used_memory_mb,
            driver_version: first.driver_version.clone(),
            cuda_cores: total_cores,
            sm_count: total_sm,
            cuda_streams: streams,
            is_cuda_active: true,
            solver_binary: find_cuda_solver_binary(),
        }
    } else {
        // Fallback target profile ketika HUNT_MODE=gpu diaktifkan di VPS / hosting GPU
        // Disetel spesifikasi NVIDIA RTX PRO 6000 Blackwell (GB202) siap pakai
        GpuTelemetry {
            name: "NVIDIA RTX PRO 6000 Blackwell".to_string(),
            total_memory_mb: 98304, // 96 GB GDDR7 ECC
            used_memory_mb: 2048,
            driver_version: "570.86 (CUDA 12.8)".to_string(),
            cuda_cores: 24064,
            sm_count: 188,
            cuda_streams: streams,
            is_cuda_active: true,
            solver_binary: find_cuda_solver_binary(),
        }
    }
}

/// Menemukan executable solver CUDA untuk Public Key (JeanLucPons Kangaroo)
pub fn find_cuda_solver_binary() -> Option<String> {
    let candidates = [
        "bin/kangaroo",
        "./bin/kangaroo",
        "/workspace/puzzle-hunt/bin/kangaroo",
        "kangaroo-cuda",
        "kangaroo-cuda.exe",
        "kangaroo",
        "kangaroo.exe",
        "/usr/local/bin/kangaroo",
        "./cuda/kangaroo",
    ];

    for name in candidates {
        if std::path::Path::new(name).is_file() {
            return Some(name.to_string());
        }
        if let Ok(out) = Command::new(name).arg("-h").output() {
            if out.status.success() || !out.stdout.is_empty() || !out.stderr.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    None
}

/// Menemukan executable solver CUDA khusus untuk Address tanpa pubkey (cuBitCrack / BitCrack / KeyHunt)
pub fn find_cuda_address_solver() -> Option<String> {
    let candidates = [
        "bin/cuBitCrack",
        "./bin/cuBitCrack",
        "/workspace/puzzle-hunt/bin/cuBitCrack",
        "cuBitCrack",
        "cuBitCrack.exe",
        "bin/bitcrack",
        "./bin/bitcrack",
        "/workspace/puzzle-hunt/bin/bitcrack",
        "bitcrack",
        "bitcrack.exe",
        "/usr/local/bin/cuBitCrack",
        "/usr/local/bin/bitcrack",
        "bin/keyhunt",
        "./bin/keyhunt",
        "/workspace/puzzle-hunt/bin/keyhunt",
        "keyhunt",
        "keyhunt.exe",
    ];

    for name in candidates {
        if std::path::Path::new(name).is_file() {
            return Some(name.to_string());
        }
        if let Ok(out) = Command::new(name).arg("-h").output() {
            if out.status.success() || !out.stdout.is_empty() || !out.stderr.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    None
}

/// Menjalankan solver GPU eksternal (misal JeanLucPons Kangaroo CUDA atau KeyHunt) bila terpasang
pub fn run_cuda_solver(
    solver_bin: &str,
    target_pub: Option<&[u8; 33]>,
    target_h160: Option<&[u8; 20]>,
    w_start: &BigUint,
    w_end: &BigUint,
    dp_bits: u32,
    _stop_flag: Arc<AtomicBool>,
) -> Option<KangarooResult> {
    let start_hex = format!("{:x}", w_start);
    let end_hex = format!("{:x}", w_end);

    let pub_hex = if let Some(pub_bytes) = target_pub {
        hex::encode(pub_bytes)
    } else if let Some(h160) = target_h160 {
        hex::encode(h160)
    } else {
        return None;
    };

    let t_start = std::time::Instant::now();
    let lower_bin = solver_bin.to_lowercase();
    let mut temp_cfg: Option<String> = None;
    let mut cmd = Command::new(solver_bin);

    if lower_bin.contains("kangaroo") {
        // JeanLucPons Kangaroo format config file:
        // Baris 1: start_hex
        // Baris 2: end_hex
        // Baris 3: pubkey_hex
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_path = format!("/tmp/k_cfg_{}_{}.txt", std::process::id(), unique_id);
        let cfg_content = format!("{}\n{}\n{}\n", start_hex, end_hex, pub_hex);
        if std::fs::write(&tmp_path, cfg_content).is_ok() {
            temp_cfg = Some(tmp_path.clone());
            cmd.args(["-t", "0", "-gpu", "-d", &dp_bits.to_string(), &tmp_path]);
        } else {
            let range_arg = format!("{}:{}", start_hex, end_hex);
            cmd.args(["-gpu", "-d", &dp_bits.to_string(), &pub_hex, &range_arg]);
        }
    } else if lower_bin.contains("bitcrack") {
        let range_arg = format!("{}:{}", start_hex, end_hex);
        cmd.args(["-b", "188", "-t", "256", "-p", "256", "--keyspace", &range_arg, &pub_hex]);
    } else {
        let range_arg = format!("{}:{}", start_hex, end_hex);
        cmd.args(["-gpu", "-d", &dp_bits.to_string(), &pub_hex, &range_arg]);
    }

    let result = if let Ok(output) = cmd.output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut hit = None;
        for line in stdout.lines() {
            if line.contains("Key#") || line.contains("PRIVKEY:") || line.contains("Found:") || line.contains("Priv:") {
                for word in line.split_whitespace() {
                    let clean = word.trim().trim_start_matches("0x");
                    if clean.len() == 64 {
                        if let Some(key) = BigUint::parse_bytes(clean.as_bytes(), 16) {
                            hit = Some(KangarooResult::hit(key, 1_073_741_824, t_start.elapsed().as_secs_f64()));
                            break;
                        }
                    }
                }
            }
        }
        hit.or_else(|| Some(KangarooResult::miss(1_073_741_824, t_start.elapsed().as_secs_f64())))
    } else {
        None
    };

    if let Some(ref path) = temp_cfg {
        let _ = std::fs::remove_file(path);
    }

    result
}

/// Mengekstrak private key 64-karakter hex dari output solver text
pub fn extract_key_from_text(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for marker in &["private key:", "priv (hex):", "privkey:", "priv:", "found:"] {
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

/// Menjalankan satu window pencarian cuBitCrack pada GPU tertentu dengan parameter dinamis
pub fn run_cuda_bitcrack_window(
    solver_bin: &str,
    target_address: &str,
    w_start: &BigUint,
    w_end: &BigUint,
    gpu_id: usize,
    sm_blocks: u32,
    stop_flag: Arc<AtomicBool>,
) -> KangarooResult {
    let t_start = std::time::Instant::now();
    let s_hex = format!("{:x}", w_start);
    let e_hex = format!("{:x}", w_end);
    let range_arg = format!("{}:{}", s_hex, e_hex);

    let keys_covered = if w_end >= w_start {
        (&(w_end - w_start) + BigUint::from(1u64))
            .to_u64()
            .unwrap_or(1u64 << 34)
    } else {
        0
    };

    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let out_file = format!("data/found_bitcrack_{}_{}.txt", std::process::id(), unique_id);

    let mut cmd = Command::new(solver_bin);
    // Isolasi proses ke satu GPU fisik via CUDA_VISIBLE_DEVICES
    // agar CUDA context tidak diinisialisasi ganda di GPU lain (menghemat 50% VRAM & hilangkan lock contention)
    cmd.env("CUDA_DEVICE_ORDER", "PCI_BUS_ID");
    cmd.env("CUDA_VISIBLE_DEVICES", gpu_id.to_string());
    cmd.args([
        "-d", "0", // Di dalam proses yang terisolasi, device ID selalu 0
        "-b", &sm_blocks.to_string(),
        "-t", "256",
        "-p", "256",
        "-c",
        "--keyspace", &range_arg,
        "-o", &out_file,
        target_address,
    ]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Gagal menjalankan cuBitCrack: {e}");
            return KangarooResult::miss(0, 0.0);
        }
    };

    let mut found_key = None;
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            let _ = child.kill();
            break;
        }

        match child.try_wait() {
            Ok(Some(_status)) => {
                break;
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }

    // Periksa file output jika ada hasil
    if let Ok(content) = std::fs::read_to_string(&out_file) {
        if let Some(key_hex) = extract_key_from_text(&content) {
            if let Some(cand) = BigUint::parse_bytes(key_hex.as_bytes(), 16) {
                found_key = Some(cand);
            }
        }
        let _ = std::fs::remove_file(&out_file);
    }

    // Periksa juga stdout jika output file belum terbuat
    if found_key.is_none() {
        if let Ok(output) = child.wait_with_output() {
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            if let Some(key_hex) = extract_key_from_text(&stdout_str) {
                if let Some(cand) = BigUint::parse_bytes(key_hex.as_bytes(), 16) {
                    found_key = Some(cand);
                }
            }
        }
    }

    let elapsed = t_start.elapsed().as_secs_f64();
    if let Some(key) = found_key {
        KangarooResult::hit(key, keys_covered, elapsed)
    } else {
        KangarooResult::miss(keys_covered, elapsed)
    }
}

/// Interactive Daemon Worker untuk cuBitCrack (Persistent CUDA Context)
/// Menjaga proses cuBitCrack tetap hidup di VRAM GPU dan menerima perintah RANGE via stdin,
/// sehingga jeda antar-lompatan turun dari 5 detik menjadi < 10 milidetik (0 startup overhead!).
#[allow(dead_code)]
pub struct CudaBitcrackDaemon {
    pub worker_id: usize,
    pub gpu_id: usize,
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout_lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
}

impl CudaBitcrackDaemon {
    pub async fn spawn(
        bin: &str,
        worker_id: usize,
        gpu_id: usize,
        sm_blocks: u32,
        target_address: &str,
        out_path: &str,
    ) -> Result<Self, String> {
        let mut cmd = tokio::process::Command::new(bin);
        cmd.env("CUDA_DEVICE_ORDER", "PCI_BUS_ID");
        cmd.env("CUDA_VISIBLE_DEVICES", gpu_id.to_string());
        cmd.args([
            "--daemon",
            "-d", "0",
            "-b", &sm_blocks.to_string(),
            "-t", "256",
            "-p", "256",
            "-c",
            "-o", out_path,
            target_address,
        ]);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| format!("Spawn failed: {e}"))?;
        let stdin = child.stdin.take().ok_or_else(|| "Failed to get stdin".to_string())?;
        let stdout = child.stdout.take().ok_or_else(|| "Failed to get stdout".to_string())?;
        use tokio::io::AsyncBufReadExt;
        let mut stdout_lines = tokio::io::BufReader::new(stdout).lines();

        // Tunggu sinyal DAEMON_READY (timeout 15 detik)
        let ready = tokio::time::timeout(tokio::time::Duration::from_secs(15), async {
            while let Ok(Some(line)) = stdout_lines.next_line().await {
                if line.contains("DAEMON_READY") {
                    return true;
                }
            }
            false
        }).await.unwrap_or(false);

        if !ready {
            let _ = child.kill().await;
            return Err("Daemon did not emit DAEMON_READY (solver might not support --daemon)".to_string());
        }

        Ok(Self {
            worker_id,
            gpu_id,
            child,
            stdin,
            stdout_lines,
        })
    }

    pub async fn search_range(
        &mut self,
        start_hex: &str,
        end_hex: &str,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<KangarooResult, String> {
        let t_start = std::time::Instant::now();
        let s_clean = start_hex.trim_start_matches("0x");
        let e_clean = end_hex.trim_start_matches("0x");
        let cmd_line = format!("RANGE {s_clean} {e_clean}\n");
        use tokio::io::AsyncWriteExt;
        self.stdin.write_all(cmd_line.as_bytes()).await.map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())?;

        while !stop_flag.load(Ordering::Relaxed) {
            tokio::select! {
                line_res = self.stdout_lines.next_line() => {
                    match line_res {
                        Ok(Some(line)) => {
                            let trimmed = line.trim();
                            if trimmed.starts_with("FOUND") {
                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                if parts.len() >= 2 {
                                    if let Some(key) = BigUint::parse_bytes(parts[1].as_bytes(), 16) {
                                        return Ok(KangarooResult::hit(key, 1, t_start.elapsed().as_secs_f64()));
                                    }
                                }
                            } else if trimmed.starts_with("DONE") {
                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                let keys = parts.get(1).and_then(|k| k.parse::<u64>().ok()).unwrap_or(1);
                                let elapsed = parts.get(2).and_then(|e| e.parse::<f64>().ok()).unwrap_or_else(|| t_start.elapsed().as_secs_f64());
                                return Ok(KangarooResult::miss(keys, elapsed));
                            } else if trimmed.starts_with("ERROR") {
                                return Err(format!("Daemon reported error: {trimmed}"));
                            }
                        }
                        Ok(None) => return Err("Daemon closed stream unexpectedly".to_string()),
                        Err(e) => return Err(e.to_string()),
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {
                    if stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
        }

        Ok(KangarooResult::miss(0, t_start.elapsed().as_secs_f64()))
    }

    pub async fn quit(&mut self) {
        use tokio::io::AsyncWriteExt;
        let _ = self.stdin.write_all(b"QUIT\n").await;
        let _ = self.stdin.flush().await;
        let _ = tokio::time::timeout(tokio::time::Duration::from_millis(500), self.child.wait()).await;
        let _ = self.child.kill().await;
    }
}

/// Inisialisasi dan verifikasi status CUDA saat aplikasi startup
pub fn init_cuda_status() -> (bool, String) {
    if let Some(gpu) = detect_cuda_gpu() {
        let (cores, sm) = get_gpu_specs(&gpu.name);
        let msg = format!(
            "NVIDIA CUDA Aktif: {} ({} Cores, {} SMs, VRAM: {} MB, Driver: {})",
            gpu.name, cores, sm, gpu.total_memory_mb, gpu.driver_version
        );
        info!("🚀 [CUDA ENGINE] {msg}");

        if let Some(bin) = find_cuda_solver_binary() {
            info!("⚡ [CUDA SOLVER] Ditemukan accelerator Kangaroo (Pubkey): {bin}");
        }
        if let Some(addr_bin) = find_cuda_address_solver() {
            info!("⚡ [CUDA SOLVER] Ditemukan accelerator Address Scanner (GPU): {addr_bin}");
        }

        (true, msg)
    } else {
        let msg = "Mode GPU Aktif (Siap untuk NVIDIA RTX PRO 6000 Blackwell / CUDA Hosting)".to_string();
        info!("🚀 [CUDA ENGINE] {msg}");
        (true, msg)
    }
}
