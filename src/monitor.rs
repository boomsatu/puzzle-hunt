//! System Hardware & Resource Monitoring (CPU & RAM Telemetry)
//! Mengukur CPU Load dan Memori secara non-blocking dan ultra-ringan
//! tanpa membebani thread komputasi Pollard's Kangaroo.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub event_type: String, // "system_metrics"
    pub cpu_usage_pct: f32,
    pub per_core_usage: Vec<f32>,
    pub cpu_cores: usize,
    pub cpu_brand: String,
    pub cpu_freq_mhz: u64,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_usage_pct: f32,
}

pub struct SystemMonitor {
    sys: Mutex<System>,
    last_metrics: Mutex<SystemMetrics>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        // Inisialisasi System HANYA untuk CPU Usage dan Memory (RAM).
        // Kita SENGAJA menonaktifkan proses, disk, dan network refresh
        // agar overhead mendekati 0 (< 0.01% CPU, sub-milidetik per sampling).
        let refresh_kind = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing().with_usage().with_frequency())
            .with_memory(MemoryRefreshKind::nothing().with_ram());

        let mut sys = match System::new_with_specifics(refresh_kind) {
            Ok(s) => s,
            Err(_) => System::new().expect("Failed to initialize sysinfo System"),
        };

        // Sampling awal
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_cores = sys.cpus().len();
        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Unknown CPU".into());
        let cpu_freq_mhz = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);
        let total_mem = sys.total_memory();
        let used_mem = sys.used_memory();

        let initial_metrics = SystemMetrics {
            event_type: "system_metrics".to_string(),
            cpu_usage_pct: 0.0,
            per_core_usage: vec![0.0; cpu_cores],
            cpu_cores,
            cpu_brand,
            cpu_freq_mhz,
            memory_used_mb: used_mem / (1024 * 1024),
            memory_total_mb: total_mem / (1024 * 1024),
            memory_usage_pct: if total_mem > 0 {
                (used_mem as f32 / total_mem as f32) * 100.0
            } else {
                0.0
            },
        };

        Self {
            sys: Mutex::new(sys),
            last_metrics: Mutex::new(initial_metrics),
        }
    }

    /// Refresh metrik CPU dan memori. Mengembalikan SystemMetrics terbaru.
    pub fn sample(&self) -> SystemMetrics {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let cpu_usage_pct = sys.global_cpu_usage();
        let per_core_usage: Vec<f32> = sys.cpus().iter().map(|c| c.usage()).collect();
        let cpu_cores = sys.cpus().len();
        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Unknown CPU".into());
        let cpu_freq_mhz = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);

        let total_mem = sys.total_memory();
        let used_mem = sys.used_memory();
        let memory_total_mb = total_mem / (1024 * 1024);
        let memory_used_mb = used_mem / (1024 * 1024);
        let memory_usage_pct = if total_mem > 0 {
            (used_mem as f32 / total_mem as f32) * 100.0
        } else {
            0.0
        };

        let metrics = SystemMetrics {
            event_type: "system_metrics".to_string(),
            cpu_usage_pct,
            per_core_usage,
            cpu_cores,
            cpu_brand,
            cpu_freq_mhz,
            memory_used_mb,
            memory_total_mb,
            memory_usage_pct,
        };

        let mut last = self.last_metrics.lock().unwrap();
        *last = metrics.clone();
        metrics
    }

    pub fn get_latest(&self) -> SystemMetrics {
        self.last_metrics.lock().unwrap().clone()
    }
}
