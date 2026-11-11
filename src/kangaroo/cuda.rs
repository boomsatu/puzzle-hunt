//! NVIDIA CUDA Hardware Acceleration Module for Bitcoin Puzzle Hunter
//! Mendukung deteksi GPU otomatis, pemanggilan solver CUDA (Kangaroo CUDA / KeyHunt / BitCrack),
//! serta diagnostic reporting untuk GPU Hosting (Hostinger, Vast.ai, RunPod, AWS EC2 G-series, dll).

use std::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct CudaGpuInfo {
    pub name: String,
    pub total_memory_mb: u64,
    pub driver_version: String,
}

/// Mendeteksi apakah sistem memiliki NVIDIA GPU dan CUDA driver yang aktif
pub fn detect_cuda_gpu() -> Option<CudaGpuInfo> {
    // 1. Eksekusi query nvidia-smi
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,driver_version",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => return None,
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?;
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

    if parts.is_empty() {
        return None;
    }

    let name = parts[0].to_string();
    let total_memory_mb = parts.get(1).and_then(|m| m.parse::<u64>().ok()).unwrap_or(0);
    let driver_version = parts.get(2).map(|d| d.to_string()).unwrap_or_else(|| "Unknown".into());

    Some(CudaGpuInfo {
        name,
        total_memory_mb,
        driver_version,
    })
}

/// Menemukan executable solver CUDA yang tersedia di PATH atau direktori proyek
pub fn find_cuda_solver_binary() -> Option<String> {
    let candidates = [
        "kangaroo-cuda",
        "kangaroo-cuda.exe",
        "kangaroo",
        "kangaroo.exe",
        "keyhunt",
        "keyhunt.exe",
        "bitcrack",
        "bitcrack.exe",
        "./cuda/kangaroo",
        "./bin/kangaroo",
    ];

    for name in candidates {
        if let Ok(out) = Command::new(name).arg("-h").output() {
            if out.status.success() || !out.stdout.is_empty() || !out.stderr.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    None
}

/// Inisialisasi dan verifikasi status CUDA saat aplikasi startup
pub fn init_cuda_status() -> (bool, String) {
    if let Some(gpu) = detect_cuda_gpu() {
        let msg = format!(
            "NVIDIA CUDA Aktif: {} (VRAM: {} MB, Driver: {})",
            gpu.name, gpu.total_memory_mb, gpu.driver_version
        );
        info!("🚀 [CUDA ENGINE] {msg}");

        if let Some(bin) = find_cuda_solver_binary() {
            info!("⚡ [CUDA SOLVER] Ditemukan accelerator binary: {bin}");
        } else {
            info!("ℹ️ [CUDA PIPELINE] Menggunakan integrated GPU dispatch pipeline.");
        }

        (true, msg)
    } else {
        let msg = "NVIDIA CUDA / GPU tidak terdeteksi pada host ini".to_string();
        warn!("⚠️ [CUDA ENGINE] {msg}");
        (false, msg)
    }
}

