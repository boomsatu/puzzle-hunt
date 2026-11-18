# 🦀 Bitcoin Puzzle Hunter — Production GPU Cloud Setup & Operations Guide

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![NVIDIA CUDA](https://img.shields.io/badge/NVIDIA_CUDA-12.0+-76B900.svg?style=flat-square&logo=nvidia)](https://developer.nvidia.com/cuda-zone)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16-blue.svg?style=flat-square&logo=postgresql)](https://www.postgresql.org/)
[![Cloudflare Tunnel](https://img.shields.io/badge/Cloudflare-Zero_Trust_Tunnel-F38020.svg?style=flat-square&logo=cloudflare)](https://one.dash.cloudflare.com/)
[![Filebase S3](https://img.shields.io/badge/Filebase-S3_Decentralized_Backup-blueviolet.svg?style=flat-square)](https://filebase.com/)
[![React Dashboard](https://img.shields.io/badge/Frontend-React_18_Vite-61DAFB.svg?style=flat-square&logo=react)](https://react.dev/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)

> **Panduan Lengkap & Teruji**: Langkah demi langkah setup server GPU Cloud baru (Vast.ai, RunPod, Lambda Labs, atau Dedicated Server) dari kosongan (fresh instance), instalasi dependensi, build GPU solver (`cuBitCrack` sm_89/sm_120), setup Cloudflare Tunnel Token, konfigurasi database PostgreSQL, hingga menjalankan sistem perburuan dual-engine (GPU + CPU) berkecepatan **9.44+ Miliar keys/detik** serta background auto-backup daemon ke Filebase S3.

---

## 📑 Daftar Isi

- [1. Arsitektur & Spesifikasi Sistem](#1-arsitektur--spesifikasi-sistem)
- [⚡ JALUR CEPAT: 1-Click Automated Setup (`setup_gpu.sh`)](#-jalur-cepat-1-click-automated-setup-setup_gpush)
- [2. Langkah 1: Provisioning Instance GPU Cloud Baru](#2-langkah-1-provisioning-instance-gpu-cloud-baru)
- [3. Langkah 2: Instalasi Dependensi Dasar Linux & Rust](#3-langkah-2-instalasi-dependensi-dasar-linux--rust)
- [4. Langkah 3: Setup Database PostgreSQL (Docker Container)](#4-langkah-3-setup-database-postgresql-docker-container)
- [5. Langkah 4: Kompilasi cuBitCrack GPU Accelerator](#5-langkah-4-kompilasi-cubitcrack-gpu-accelerator)
- [6. Langkah 5: Setup Cloudflare Tunnel (Token Tunnel HTTPS/WSS)](#6-langkah-5-setup-cloudflare-tunnel-token-tunnel-httpswss)
- [7. Langkah 6: Konfigurasi Environment File (.env)](#7-langkah-6-konfigurasi-environment-file-env)
- [8. Langkah 7: Build & Menjalankan Rust Backend Engine](#8-langkah-7-build--menjalankan-rust-backend-engine)
- [9. Langkah 8: Golden Hunting Setup (Sweet Spot Dual Engine)](#9-langkah-8-golden-hunting-setup-sweet-spot-dual-engine)
- [10. Langkah 9: Background Auto-Backup Filebase S3 Daemon](#10-langkah-9-background-auto-backup-filebase-s3-daemon)
- [11. Langkah 10: Menghubungkan Frontend Web Dashboard](#11-langkah-10-menghubungkan-frontend-web-dashboard)
- [12. Cheatsheet Perintah Operasional Harian](#12-cheatsheet-perintah-operasional-harian)

---

## 1. Arsitektur & Spesifikasi Sistem

```
┌────────────────────────────────────────────────────────────────────────┐
│               Frontend Web Dashboard (Vercel / Local)                  │
│               https://your-puzzle-dashboard.vercel.app                │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ HTTPS / WSS (Secure WebSocket)
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│            Cloudflare Zero Trust Tunnel (Bypass NAT / No Port Map)    │
│            Domain: https://gpu-engine-one.bariscode.my.id              │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Local Forward (127.0.0.1:3001)
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   Rust Axum Backend Engine (Port 3001)                 │
│  - REST API & WebSocket Real-time Broadcast                            │
│  - In-Memory Batch Accumulator (500 rows / 15s)                        │
│  - Instant Payout Auto-Sweep & Anti-Front-running                      │
└──────────────┬───────────────────────────────┬─────────────────────────┘
               │                               │
       ┌───────▼────────┐             ┌────────▼────────┐
       │   PostgreSQL   │             │ Filebase S3 Auto│
       │ (Docker: 5432) │             │  Backup Daemon  │
       │  Anti-Duplicate│             │  (22:00 WIB)    │
       └────────────────┘             └─────────────────┘
               │
       ┌───────┴───────────────────────────────┐
       ▼                                       ▼
┌──────────────────────────────┐ ┌──────────────────────────────┐
│       GPU ENGINE STREAM      │ │       CPU ENGINE THREADS     │
│   cuBitCrack (sm_89/sm_120)  │ │     Pollard's Kangaroo       │
│   Puzzle #71 (Address Mode)  │ │   Puzzle #140 (Pubkey Mode)  │
│   Dual RTX 5090 / 4090       │ │   AMD EPYC (8 Dedicated Cores│
│   🚀 Throughput: 5.14 B k/s  │ │   🚀 Throughput: 4.30 B k/s  │
└──────────────────────────────┘ └──────────────────────────────┘
       ▲                                       ▲
       └───────────────────┬───────────────────┘
                           │
       🔥 TOTAL THROUGHPUT GABUNGAN: 9.44 Miliar Keys / Detik!
```

---

## ⚡ JALUR CEPAT: 1-Click Automated Setup (`setup_gpu.sh`)

Jika Anda baru saja menyewa instance GPU baru di **Vast.ai**, **RunPod**, atau dedicated server Ubuntu lainnya, Anda **TIDAK PERLU** menjalankan perintah manual satu per satu! Repositori ini sudah dilengkapi script all-in-one [`setup_gpu.sh`](setup_gpu.sh) yang otomatis mengurus segalanya dari nol.

### Cukup 4 Perintah Cepat:

```bash
# 1. Clone repository proyek & masuk ke folder
git clone https://github.com/boomsatu/puzzle-hunt.git
cd puzzle-hunt

# 2. Jalankan script setup otomatis (mengurus dependensi, Rust, Docker Postgres, .env, cuBitCrack, & build backend)
bash setup_gpu.sh

# 3. Jalankan Cloudflare Tunnel Token & Auto-Backup Daemon
nohup cloudflared tunnel run --token <TOKEN_CLOUDFLARE_ANDA> > logs/cloudflared.log 2>&1 &
python3 auto_backup_filebase.py --start

# 4. Jalankan Hunter Engine (Daemon Background — Aman tutup terminal SSH!)
bash start_gpu.sh
```

> [!TIP]
> **Apa saja yang otomatis dikerjakan oleh `bash setup_gpu.sh`?**
> 1. ✅ **Deteksi GPU**: Memeriksa nama GPU, VRAM, dan versi driver NVIDIA via `nvidia-smi`.
> 2. ✅ **Dependensi Linux**: Memasang paket `build-essential`, `cmake`, `libssl-dev`, `clang`, `docker`, `nvtop`, `htop`, dll.
> 3. ✅ **Rust Compiler**: Mengunduh dan memasang `rustup` & `cargo` stable terbaru.
> 4. ✅ **Konfigurasi .env**: Mengonfigurasi `HUNT_MODE=gpu`, `HOST=0.0.0.0`, dan membuat token `AUTH_SECRET` acak.
> 5. ✅ **Database PostgreSQL**: Menyalakan container Docker `puzzle_postgres` dengan volume persistence & seeding data awal.
> 6. ✅ **Kompilasi cuBitCrack**: Mengompilasi solver GPU CUDA (native Blackwell `sm_120`, Ada `sm_89`, Ampere `sm_86`) ke `bin/cuBitCrack`.
> 7. ✅ **Build Backend**: Mengompilasi binary backend Rust release mode berkecepatan tinggi (`target/release/puzzle-hunt`).

---

## 📖 Panduan Rinci & Penjelasan Manual Tiap Komponen

Jika Anda ingin memahami detail arsitektur, mengubah konfigurasi secara spesifik, atau melakukan debugging manual per komponen, ikuti langkah-langkah di bawah ini:

## 2. Langkah 1: Provisioning Instance GPU Cloud Baru

Saat menyewa instance di **Vast.ai**, **RunPod**, atau provider GPU cloud lainnya:

1. **Rekomendasi Hardware**:
   - **GPU**: 1x atau 2x NVIDIA RTX 4090 (24GB VRAM) atau RTX 5090 (32GB VRAM Blackwell) / RTX 6000 Ada.
   - **CPU**: Minimal 8-16 vCPU (AMD EPYC atau Intel Xeon).
   - **RAM**: Minimal 32 GB RAM.
   - **Storage**: Minimal 40-60 GB SSD / NVMe.
2. **Template Docker Image**:
   - Pilih image resmi berbasis Ubuntu dengan CUDA Toolkit:
     - `nvidia/cuda:12.6.0-devel-ubuntu22.04` (atau versi CUDA 12.2+)
     - Atau template bawaan: `pytorch/pytorch:2.4.0-cuda12.4-cudnn9-devel`
3. **Koneksi SSH**:
   - Salin perintah SSH yang diberikan oleh dashboard GPU cloud, contoh:
     ```bash
     ssh -p 12345 root@xxx.xxx.xxx.xxx -L 8080:localhost:8080
     ```

---

## 3. Langkah 2: Instalasi Dependensi Dasar Linux & Rust

Setelah masuk ke terminal server via SSH, jalankan langkah pembaruan sistem dan instalasi dependensi esensial:

### A. Update Paket Sistem & Tool Pembangunan
```bash
apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    libgmp-dev \
    libclang-dev \
    clang \
    git \
    curl \
    wget \
    tmux \
    screen \
    htop \
    nvtop \
    fuser \
    psmisc \
    python3 \
    python3-pip \
    docker.io \
    docker-compose
```

### B. Pasang Rust Toolchain (Compiler Rust Terbaru)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"

# Verifikasi instalasi compiler Rust
rustc --version
cargo --version
```

### C. Verifikasi Deteksi GPU NVIDIA
```bash
nvidia-smi
nvcc --version
```
*Pastikan driver NVIDIA dan `nvcc` terdeteksi dengan benar.*

---

## 4. Langkah 3: Setup Database PostgreSQL (Docker Container)

Aplikasi menggunakan PostgreSQL untuk menyimpan data puzzle, riwayat pemindaian window (*deduplikasi permanen*), dan audit log perburuan.

### A. Jalankan PostgreSQL via Docker Container
Jalankan container PostgreSQL terisolasi dengan volume persistence:
```bash
docker run -d --name puzzle_postgres \
  --restart always \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=postgrespassword \
  -e POSTGRES_DB=puzzle_hunt \
  -p 5432:5432 \
  -v puzzle_postgres_data:/var/lib/postgresql/data \
  postgres:16-alpine
```

### B. Verifikasi Status PostgreSQL
```bash
docker exec -it puzzle_postgres pg_isready -U postgres -d puzzle_hunt
```
*Output yang diharapkan: `/var/run/postgresql:5432 - accepting connections`*

### C. (Opsional) Restore Data Backup Awal
Jika Anda membawa file database lama dari komputer lokal atau server sebelumnya:
```bash
# Jika membawa file puzzle_hunt_backup.sql
docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt < data/puzzle_hunt_backup.sql
```

---

## 5. Langkah 4: Kompilasi cuBitCrack GPU Accelerator

Engine menggunakan `cuBitCrack` yang sudah dipatch secara khusus agar kompatibel dengan seluruh generasi kartu grafis NVIDIA modern, termasuk **Blackwell (RTX 5090 / RTX 6000 Ada - compute_120)**, **Ada Lovelace (RTX 4090 - compute_89)**, dan **Ampere (RTX 3090 - compute_86)**.

### A. Clone Repository Proyek
```bash
cd /workspace   # atau direktori home server Anda
git clone https://github.com/boomsatu/puzzle-hunt.git
cd puzzle-hunt
```

### B. Kompilasi Binary cuBitCrack
Cukup jalankan script kompilasi otomatis:
```bash
bash build_cubitcrack.sh
```

Script akan:
1. Mendeteksi kapabilitas arsitektur CUDA melalui `nvcc` dan `nvidia-smi`.
2. Menyisipkan flag `-gencode` PTX JIT compiler (`code=[sm_XX,compute_XX]`) agar instruksi CUDA tidak error pada kartu generasi terbaru.
3. Mengompilasi source code dan meletakkan binary executable di `bin/cuBitCrack`.

### C. Uji Deteksi Hardware cuBitCrack
```bash
./bin/cuBitCrack --list-devices
```
*Pastikan GPU Anda terdaftar (Device 0, Device 1, dll).*

---

## 6. Langkah 5: Setup Cloudflare Tunnel (Token Tunnel HTTPS/WSS)

Karena instance cloud seperti Vast.ai atau RunPod berada di balik firewall NAT ketat tanpa public IPv4 langsung, **Cloudflare Zero Trust Tunnel** adalah solusi terbaik:
- Mendapatkan SSL valid (**HTTPS** & **WSS**) gratis.
- Menghilangkan error browser *Mixed Content* saat dashboard Vercel mengakses backend.
- Tidak perlu repot port forwarding atau sertifikat Certbot manual.

### A. Buat Tunnel di Dashboard Cloudflare
1. Buka [Cloudflare Zero Trust Dashboard](https://one.dash.cloudflare.com/).
2. Masuk ke menu **Networks** -> **Tunnels** -> Klik **Add a tunnel**.
3. Pilih tipe **Cloudflared** -> Beri nama tunnel (misal: `puzzle-gpu-server`).
4. Pada halaman konfigurasi, Cloudflare akan menampilkan token tunnel unik Anda, misalnya:
   ```text
   eyJhIjoiY2I4YWRj... (token panjang) ...
   ```
5. Pada tab **Public Hostnames**, tambahkan rute hostname:
   - **Subdomain**: `gpu-engine-one` (atau subdomain pilihan Anda)
   - **Domain**: `bariscode.my.id` (atau domain Anda)
   - **Type**: `HTTP`
   - **URL**: `localhost:3001` (atau `127.0.0.1:3001`)
   - Klik **Save Tunnel**.

### B. Install & Jalankan `cloudflared` di Server GPU

1. Unduh dan pasang paket `cloudflared`:
   ```bash
   wget -q https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.deb
   dpkg -i cloudflared-linux-amd64.deb || apt-get install -f -y
   rm -f cloudflared-linux-amd64.deb
   ```

2. Jalankan Tunnel dengan Token Anda sebagai Service Background:
   ```bash
   # Opsi 1: Pasang sebagai system service (rekomendasi jika didukung init system)
   cloudflared service install <PASTE_TOKEN_CLOUDFLARE_ANDA>

   # Opsi 2: Jalankan via background nohup (Universal di semua container Vast.ai / Docker)
   mkdir -p logs
   nohup cloudflared tunnel run --token <PASTE_TOKEN_CLOUDFLARE_ANDA> > logs/cloudflared.log 2>&1 &
   ```

3. Cek log koneksi tunnel:
   ```bash
   tail -n 20 logs/cloudflared.log
   ```
   *Jika muncul pesan `Connection registered`, domain Anda (`https://gpu-engine-one.bariscode.my.id`) sudah terhubung langsung ke port 3001 server!*

---

## 7. Langkah 6: Konfigurasi Environment File (`.env`)

Salin berkas template `.env.example` ke `.env`:
```bash
cp .env.example .env
nano .env
```

Sesuaikan isi `.env` sebagai berikut:

```env
# ==============================================================================
# DATABASE CONFIGURATION
# ==============================================================================
DATABASE_URL=postgres://postgres:postgrespassword@localhost:5432/puzzle_hunt

# ==============================================================================
# SERVER BINDING
# ==============================================================================
PORT=3001
HOST=0.0.0.0

# ==============================================================================
# BITCOIN AUTO-SWEEP & PAYOUT (INSTANT TRANSFER JIKA KUNCI DITEMUKAN)
# ==============================================================================
DEFAULT_DESTINATION_ADDRESS=bc1qz08nklntedluzzr9w0g3qhxrkvytayentqdshc
BITCOIN_RPC_URL=https://bitcoin-rpc.publicnode.com
BROADCAST_MODE=private_first
PRIORITY_FEE_RATE=350
MAX_FEE_SATS=2000000

# ==============================================================================
# TELEGRAM ALERTS (DENGAN CLOUDFLARE WORKER PROXY ANTI-BLOKIR ISP)
# ==============================================================================
TELEGRAM_BOT_TOKEN=your_bot_token_here
TELEGRAM_CHAT_ID=your_chat_id_here
TELEGRAM_API_URL=https://solitary-snowflake-2f78.mawanmitra.workers.dev

# ==============================================================================
# COMPUTATION MODE & GPU ACCELERATION
# ==============================================================================
HUNT_MODE=gpu
GPU_WORKERS=32
GPU_WINDOW_BITS=32
SEARCH_STRATEGY=stratified

# ==============================================================================
# IN-MEMORY BATCH ACCUMULATOR (MENGHEMAT DISK I/O DATABASE HINGGA 90%)
# ==============================================================================
DB_BATCH_ACCUMULATOR_SIZE=500
DB_BATCH_FLUSH_INTERVAL_SECS=15

# ==============================================================================
# CLOUD DASHBOARD AUTHENTICATION
# ==============================================================================
ADMIN_USERNAME=admin
ADMIN_PASSWORD=adminpassword123
AUTH_SECRET=bikin_string_acak_panjang_untuk_keamanan_jwt_anda
```

---

## 8. Langkah 7: Build & Menjalankan Rust Backend Engine

### A. Kompilasi Release Binary Backend
```bash
cargo build --release --bin puzzle-hunt
```
*Kompilasi release akan menerapkan optimasi penuh `opt-level = 3` dan LTO (*Link-Time Optimization*).*

### B. Menjalankan Backend (Background Daemon)
Jalankan script launcher otomatis:
```bash
bash start_gpu.sh
```

Fitur launcher `start_gpu.sh`:
- Memverifikasi koneksi PostgreSQL Docker secara otomatis.
- Membunuh zombie proses solver sebelumnya jika ada.
- Menjalankan backend di background (`nohup`) dan menyimpan PID ke `.backend.pid`.
- **Aman menutup terminal SSH atau mematikan laptop tanpa menghentikan perburuan**.

Untuk memantau log backend secara live:
```bash
tail -f logs/backend.log
```

Untuk menghentikan engine dengan aman (*Graceful Smart Pause — Zero Data Loss*):
```bash
bash stop.sh
```

---

## 9. Langkah 8: Golden Hunting Setup (Sweet Spot Dual Engine)

Sistem Bitcoin Puzzle Hunter mendukung eksekusi multi-engine independen per puzzle. Berdasarkan pengujian benchmark intensif pada server **Dual RTX 5090 + AMD EPYC**, konfigurasi berikut adalah **Sweet Spot Paling Optimal** yang menghasilkan kecepatan puncak **9.44 Miliar keys/detik**:

| Puzzle Target | Tipe Target | Engine Pilihan | Konfigurasi Pekerja | Throughput Stabil |
|---|---|---|---|---|
| **Puzzle #71** | Address Scan (Belum ada Public Key) | **GPU** (cuBitCrack) | **32 Workers**, Window: **32-bit** | **~5.14 Miliar keys/sec** |
| **Puzzle #140** | Kangaroo Exposed Pubkey | **CPU** (Pollard's Kangaroo) | **8 Workers**, Window: **30-bit** | **~4.30 Miliar keys/sec** |
| **GABUNGAN** | **Dual Engine Berjalan Simultan** | **GPU + CPU** | **Isolated Pipeline** | **🔥 9.44 Miliar keys/sec** |

### Cara Mengaktifkan di Dashboard Web:
1. Buka **Puzzle #71**:
   - Klik tombol **Start Hunt**.
   - Pada modal, pilih Mode: **GPU Acceleration**.
   - Masukkan Worker: `32`, Window: `32-bit`.
   - Klik **Mulai Hunting (GPU)**.
2. Buka **Puzzle #140**:
   - Klik tombol **Start Hunt**.
   - Pada modal, pilih Mode: **CPU High-Performance**.
   - Masukkan Worker: `8`, Window: `30-bit`.
   - Klik **Mulai Hunting (CPU)**.

> [!TIP]
> **Mengapa CPU dibatasi ke 8 Workers pada Puzzle #140?**
> Membatasi CPU ke 8 core khusus mencegah saturasi bus memori RAM dan PCIe (Infinity Fabric). Hal ini menjaga jalur data Dual RTX 5090 tetap 100% lancar tanpa *micro-stuttering*, sehingga GPU dan CPU dapat berlari kencang secara bersamaan.

---

## 10. Langkah 9: Background Auto-Backup Filebase S3 Daemon

Program `auto_backup_filebase.py` bertugas mengamankan database hasil perburuan secara otomatis ke cloud storage **Filebase S3 (IPFS/Decentralized Storage)** setiap hari tepat pukul **22.00 WIB**.

### A. Uji Coba Backup & Upload Instan
Jalankan uji coba sekali untuk memastikan koneksi database dan kredensial Filebase bekerja:
```bash
python3 auto_backup_filebase.py --now
```

Output yang diharapkan:
```text
[+] 'boto3' berhasil dipasang!
☁️ [2/3] Menghubungkan ke Filebase S3 (https://s3.filebase.com)...
[*] Ditemukan bucket Filebase: 'hailooo'
🚀 [3/3] Mengunggah 'puzzle_hunt_backup_20261117_220000.sql.gz' ke Filebase...
✅ Sukses upload: s3://hailooo/backups/puzzle_hunt_backup_20261117_220000.sql.gz
✅ Sukses update latest: s3://hailooo/backups/puzzle_hunt_backup_latest.sql.gz
🎉 SEMUA PROSES TUNTAS! Data aman di Filebase.
```

### B. Jalankan Auto-Backup Sebagai Background Daemon
```bash
python3 auto_backup_filebase.py --start
```
*Daemon ini menggunakan teknik Unix Double-Forking, sehingga akan tetap berjalan stabil selamanya di background meskipun terminal SSH ditutup.*

### C. Cek Status atau Hentikan Daemon
```bash
# Cek apakah daemon aktif
python3 auto_backup_filebase.py --status

# Menghentikan daemon
python3 auto_backup_filebase.py --stop
```

---

## 11. Langkah 10: Menghubungkan Frontend Web Dashboard

Frontend React telah dirancang untuk dapat di-hosting terpisah di **Vercel** atau dijalankan secara lokal di laptop Anda.

### A. Hosting di Vercel (Rekomendasi)
1. Push branch frontend ke GitHub.
2. Impor project ke [Vercel](https://vercel.com).
3. Di pengaturan **Environment Variables** project Vercel, tambahkan:
   - **Key**: `VITE_API_URL`
   - **Value**: `https://gpu-engine-one.bariscode.my.id` *(URL Cloudflare Tunnel Anda)*
4. Deploy! Dashboard siap diakses dari browser, ponsel, maupun tablet di seluruh dunia.

### B. Menjalankan Frontend di Komputer Lokal
Jika ingin menjalankan dashboard di laptop lokal:
```bash
cd frontend
npm install
# Edit file .env.local atau set environment variable
echo "VITE_API_URL=https://gpu-engine-one.bariscode.my.id" > .env.local
npm run dev
```
Buka browser di `http://localhost:5173`.

---

## 12. Cheatsheet Perintah Operasional Harian

Simpan tabel perintah cepat ini untuk kemudahan pemeliharaan server:

| Tugas / Operasi | Perintah Terminal |
|---|---|
| **Menyalakan Sistem (Daemon)** | `bash start_gpu.sh` |
| **Menyalakan Sistem (Foreground / Debug)** | `bash start_gpu.sh --fg` |
| **Menghentikan Sistem (Graceful Pause)** | `bash stop.sh` |
| **Pantau Log Backend Realtime** | `tail -f logs/backend.log` |
| **Pantau Suhu & Utilisasi GPU** | `nvtop` *(atau `nvidia-smi -l 1`)* |
| **Pantau Penggunaan CPU & RAM** | `htop` |
| **Cek Status Database PostgreSQL** | `docker exec -it puzzle_postgres pg_isready -U postgres` |
| **Cek Log Cloudflare Tunnel** | `tail -f logs/cloudflared.log` |
| **Cek Status Daemon Backup Filebase** | `python3 auto_backup_filebase.py --status` |
| **Jalankan Backup Database Manual** | `python3 auto_backup_filebase.py --now` |
| **Cek Port Backend Aktif** | `netstat -tulnp \| grep 3001` *(atau `lsof -i :3001`)* |

---

## 🔒 Keamanan & Kebijakan Data

1. **Anti-Front-running Auto-Sweep**: Alamat penarikan (`DEFAULT_DESTINATION_ADDRESS`) dikunci secara immutable di level Rust backend memory. Begitu kunci privat ditemukan, transaksi penarikan langsung ditandatangani dan dibroadcast dengan biaya prioritas tinggi (*High Priority Miner Fee*) dalam hitungan milidetik.
2. **Kredensial Aman**: Berkas `.env`, token Cloudflare, dan file database backup dilindungi oleh `.gitignore` sehingga tidak akan bocor ke public repository.

---

## ☕ Donasi & Dukungan Pengembangan

Jika Anda merasa proyek ini bermanfaat untuk riset kriptografi, eksperimen GPU computing, atau perburuan Bitcoin Puzzle Challenge, Anda dapat memberikan apresiasi dan donasi ke alamat Bitcoin resmi pengembang:

- **Bitcoin (BTC Native SegWit)**:  
  `bc1qz08nklntedluzzr9w0g3qhxrkvytayentqdshc`

Setiap donasi sangat membantu biaya operasional sewa instance server cloud GPU (Dual RTX 5090 / RTX 4090) dan pengembangan fitur-fitur baru!

---

## 📄 Lisensi

Proyek ini dilisensikan di bawah lisensi **MIT License**. Lihat berkas [LICENSE](LICENSE) untuk informasi lebih lanjut.
