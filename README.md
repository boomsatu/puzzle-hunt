# 🦀 Bitcoin Puzzle Hunter

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16-blue.svg?style=flat-square&logo=postgresql)](https://www.postgresql.org/)
[![NVIDIA CUDA](https://img.shields.io/badge/NVIDIA_CUDA-Ready-76B900.svg?style=flat-square&logo=nvidia)](https://developer.nvidia.com/cuda-zone)
[![React](https://img.shields.io/badge/React-18-61DAFB.svg?style=flat-square&logo=react)](https://react.dev/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-3-38B2AC.svg?style=flat-square&logo=tailwind-css)](https://tailwindcss.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)

**Bitcoin Puzzle Hunter** adalah platform pemecah [Bitcoin Challenge Transaction Puzzles](https://blockchain.com) berkinerja tinggi kelas enterprise. Dibangun dari awal menggunakan **Rust** murni dengan operasi titik kurva eliptik (*secp256k1*), akselerasi multi-thread dinamis, basis data **PostgreSQL** untuk pelacakan anti-duplikasi permanen, arsitektur multi-mode (`local`, `vps`, `gpu`), serta **Web UI Dashboard** real-time interaktif berbasis React & WebSockets.

---

## ⚡ Fitur Utama & Keunggulan

### 1. ⚙️ Fleksibilitas Mode Eksekusi (`HUNT_MODE`)
Sistem mendukung tiga mode komputasi yang dapat disetel langsung melalui file `.env`:
- **Mode `local` (Default - PC Desktop)**:
  - Mengalokasikan resource secara seimbang.
  - Secara otomatis **menyisakan 2 core CPU** untuk OS, browser, PostgreSQL, dan background tasks (maksimal 8 worker threads per puzzle). Komputer tetap nyaman digunakan untuk bekerja/browsing tanpa freeze.
- **Mode `vps` (Headless Linux Cloud Server)**:
  - Dioptimalkan khusus untuk server cloud / VPS (Ubuntu/Debian).
  - Menyisakan **hanya 1 core** untuk OS + PostgreSQL + Web API, sisa seluruh core CPU (misal: 7 dari 8 core, 15 dari 16 core) didedikasikan penuh untuk pemecahan puzzle.
- **Mode `gpu` / `cuda` (Akselerasi Hardware NVIDIA GPU)**:
  - Mendeteksi ketersediaan NVIDIA GPU dan versi driver secara otomatis via query `nvidia-smi` (Nama GPU, Total VRAM, Driver Version).
  - Mendukung deteksi dan integrasi solver CUDA biner eksternal (`kangaroo-cuda`, `keyhunt`, `bitcrack`) maupun pipeline integrated GPU dispatch.
  - Memiliki fitur *fail-safe automatic fallback*: bila mode GPU diaktifkan di host tanpa GPU, sistem secara cerdas beralih ke alokasi maksimal mode VPS.
  - Siap dideploy pada layanan GPU Cloud Hosting seperti **Vast.ai, RunPod, AWS EC2 G-Series, Hostinger GPU**, dll.

### 2. ⏸️ Jeda Hunter Smart (*Graceful Smart Pause*)
- Berbeda dari sistem pembunuh thread paksa yang rentan merusak data, fitur **Jeda Hunter Smart** memastikan **Zero Data Loss**.
- Saat pengguna mengklik tombol **"Jeda Hunter"**, status hunter berubah menjadi `Pausing` (*"MENJEDA... Menyimpan Riwayat Window"*).
- Worker yang sedang berjalan dibiarkan menyelesaikan komputasi window aktifnya hingga tuntas.
- Seluruh riwayat window dan metrik langsung disimpan secara aman ke database PostgreSQL (`scanned_windows`).
- Setelah tersimpan, status berubah menjadi `Paused` dan worker berhenti dengan tertib.
- Tersedia endpoint darurat `POST /api/hunt/:number/force_stop` jika diperlukan penghentian seketika.

### 3. 🎉 Real-Time Celebration & Auto-Transfer Timeline
- **Modal Perayaan Kemenangan Interaktif**: Dilengkapi dengan animasi partikel confetti dinamis saat sebuah private key berhasil ditemukan.
- **Efek Audio Fanfare Sintesis**: Menggunakan **Web Audio API** native untuk memutar melodi kemenangan kemenangan arpeggio (*C-E-G-C-E-G-C*), bekerja instan di browser tanpa memerlukan file aset audio eksternal.
- **Pelacakan Timeline Real-Time (WebSocket Live)**: Backend Rust menyiarkan setiap tahapan proses auto-transfer secara langsung ke frontend:
  1. 🔑 **Kunci Ditemukan**: Penemuan private key, konversi ke format WIF, dan verifikasi alamat.
  2. 📝 **Penyiapan Transaksi Instan Sweep**: Mengambil UTXO dan menyusun *raw transaction*.
  3. ✍️ **Penandatanganan Digital**: Penandatanganan cryptographic signature (*secp256k1*).
  4. 📡 **Siaran ke Jaringan Bitcoin**: Pengiriman transaksi melalui RPC node publik terdistribusi (`bitcoin-rpc.publicnode.com`).
  5. ⛏️ **Mempool & Konfirmasi Blok**: Menampilkan TxID resmi dengan link langsung ke Blockchain.com explorer.
- **Fitur Simulasi**: Tombol *"Simulasikan Kemenangan"* di halaman detail untuk menguji respons UI, audio, dan alur timeline kapan saja.

### 4. 🦘 Dual-Engine Hunting Berkinerja Tinggi
- **Pollard's Kangaroo Engine $O(\sqrt{N})$**:
  - Dikhususkan untuk puzzle yang memiliki Public Key terbuka (#140, #145, #150, #155, #160).
  - Mampu memeriksa window berukuran **1 Miliar kunci hanya dalam ~1,5 detik** per thread.
- **Address Hash160 Scanner Engine**:
  - Untuk puzzle yang public key-nya belum terekspos (#66, #67, dll).
  - Melakukan pemindaian sekuensial rapat ($P_{i+1} = P_i + G$) berbasis hash160 berkecepatan tinggi.

### 5. 🛡️ Anti-Duplikasi Permanen (PostgreSQL)
- Setiap rentang window (`[w_start, w_end]`) yang telah diperiksa dicatat ke PostgreSQL dengan index B-Tree.
- Pemilihan rentang acak (*Random Jump Monte Carlo*) menjamin **100% daya komputasi selalu mengeksplorasi ruang kunci baru**, tanpa membuang siklus komputasi pada window yang pernah discan sebelumnya.

### 6. 🚀 Auto-Sweep Instan Anti Front-Running
- Alamat tujuan penarikan otomatis dikonfigurasi melalui `.env` dan terkunci permanen di level backend.
- Menggunakan **High Priority Miner Fee** (2x `estimatesmartfee(1)` atau minimal 35 sat/vB) untuk memastikan transaksi diproses miner di blok pertama tanpa risiko disalip bot front-runner.

### 7. 📊 Tampilan Dashboard Modern & Terisolasi
- **Mode Tampilan Ganda**: Beralih antara **Grid View** (tampilan kartu visual) dan **Table View** (tampilan tabel ringkas), preferensi tersimpan di `localStorage`.
- **Filter Cerdas**: Filter cepat antara *Semua Puzzle*, *Pubkey Ada (Kangaroo)*, dan *Pubkey Belum Ada (Address Mode)*.
- **Halaman Detail Terisolasi**: Panel metrik global hanya muncul di beranda utama; halaman detail menyajikan visualisasi posisi titik lompatan multi-thread (*Core Jump Positions*), metrik live, log aktivitas, dan riwayat window yang bersih.
- **Presisi Matematika 160-bit**: Format persentase desimal presisi tinggi menggunakan BigInt tanpa notasi ilmiah e-minus.

---

## 🏗️ Arsitektur Sistem

```
┌─────────────────────────────────────────────────────────────┐
│                 React + Vite Web Dashboard                  │
│                 (Port 5173 / WebSocket WSS)                 │
└──────────────────────────────┬──────────────────────────────┘
                               │ REST API & WebSocket (/api/ws)
┌──────────────────────────────▼──────────────────────────────┐
│                 Axum Web Server (Port 3001)                 │
└──────┬───────────────────────┬───────────────────────┬──────┘
       ▼                       ▼                       ▼
┌──────────────┐        ┌──────────────┐        ┌──────────────┐
│ Dynamic Hunt │        │  PostgreSQL  │        │Auto-Transfer │
│   Manager    │        │   Database   │        │    Engine    │
│(Device Modes)│        │  (Anti-Dup)  │        │(RPC + Miner) │
└──────┬───────┘        └──────────────┘        └──────┬───────┘
       │                                               │
       ├───────────────────────┬───────────────────────┤
       ▼                       ▼                       ▼
┌──────────────┐        ┌──────────────┐        ┌──────────────┐
│ Pollard's    │        │   Address    │        │ Telegram /   │
│ Kangaroo     │        │   Hash160    │        │ Celebration  │
│ (CPU / CUDA) │        │   Scanner    │        │ Timeline WS  │
└──────────────┘        └──────────────┘        └──────────────┘
```

---

## ⚙️ Konfigurasi Environment (`.env`)

Salin template `.env.example` ke `.env`:

```bash
cp .env.example .env      # Linux / macOS
copy .env.example .env    # Windows PowerShell / CMD
```

Berikut parameter konfigurasi yang tersedia:

| Variabel | Deskripsi | Nilai Contoh |
|---|---|---|
| `DATABASE_URL` | Koneksi PostgreSQL (Docker / Local) | `postgres://postgres:postgrespassword@localhost:5432/puzzle_hunt` |
| `PORT` | Port server REST API & WebSocket Axum | `3001` |
| `HOST` | Host binding interface server | `0.0.0.0` |
| `DEFAULT_DESTINATION_ADDRESS` | Alamat Bitcoin tujuan sweep instan (Terkunci permanen) | `bc1q8c0uc752amr7v4amswn5vhes37a6ffy4ral6ek` |
| `BITCOIN_RPC_URL` | Endpoint node Bitcoin JSON-RPC untuk broadcast raw tx | `https://bitcoin-rpc.publicnode.com` |
| `TELEGRAM_BOT_TOKEN` | Token Bot Telegram dari [@BotFather](https://t.me/BotFather) (Opsional) | `123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ` |
| `TELEGRAM_CHAT_ID` | Chat ID Telegram penerima notifikasi (Opsional) | `987654321` |
| `HUNT_MODE` | **Mode Komputasi**: `local` (PC), `vps` (Server Cloud), `gpu` (NVIDIA CUDA) | `local` |

---

## 🚀 Panduan Memulai Cepat

### 1. Prasyarat Sistem
- **Rust Toolchain**: v1.75+ ([rustup.rs](https://rustup.rs/))
- **Docker & Docker Compose**: Untuk menjalankan instance PostgreSQL ([docker.com](https://www.docker.com/))
- **Node.js**: v18+ & npm ([nodejs.org](https://nodejs.org/))
- *(Opsional)* **NVIDIA GPU Driver & CUDA Toolkit**: Diperlukan jika menggunakan `HUNT_MODE=gpu`

### 2. Kloning Repositori
```bash
git clone https://github.com/boomsatu/puzzle-hunt.git
cd puzzle-hunt
```

### 3. Konfigurasi File Environment
```bash
# Windows
copy .env.example .env

# Linux / macOS
cp .env.example .env
```
*Buka `.env` dan atur `HUNT_MODE` sesuai mesin Anda (`local`, `vps`, atau `gpu`), serta tentukan alamat Bitcoin tujuan transfer Anda.*

### 4. Nyalakan Database PostgreSQL
Jalankan service database menggunakan Docker Compose:
```bash
docker compose up -d postgres
```
*Database PostgreSQL akan berjalan di port `5432` dengan tabel dan relasi yang termigrasi otomatis saat backend pertama kali start.*

### 5. Kompilasi & Jalankan Backend Rust
Gunakan optimasi level rilis penuh (`--release`) dengan LTO:
```bash
cargo build --release

# Menjalankan binary server
# Windows:
.\target\release\puzzle-hunt.exe

# Linux / macOS:
./target/release/puzzle-hunt
```
*Backend API & WebSocket server akan aktif di `http://localhost:3001`.*

### 6. Jalankan Frontend Web Dashboard
Buka terminal baru di direktori `frontend`:
```bash
cd frontend
npm install
npm run dev
```
*Akses dashboard melalui browser di `http://localhost:5173`.*

---

## ☁️ Panduan Deploy di Cloud VPS & GPU Hosting

### A. Deploy di Linux VPS Headless (DigitalOcean / Linode / Contabo / Hetzner)
1. Atur `.env`:
   ```ini
   HUNT_MODE=vps
   HOST=0.0.0.0
   PORT=3001
   ```
2. Jalankan database: `docker compose up -d postgres`
3. Kompilasi: `cargo build --release`
4. Jalankan sebagai background service menggunakan `systemd` atau `tmux`.
5. Build frontend production: `cd frontend && npm run build` (dapat disajikan langsung via Nginx atau static server).

### B. Deploy di GPU Cloud (Vast.ai / RunPod / AWS EC2 G4/G5)
1. Pastikan driver NVIDIA aktif: `nvidia-smi`
2. Atur `.env`:
   ```ini
   HUNT_MODE=gpu
   ```
3. Saat backend startup, log akan memverifikasi GPU Anda:
   ```text
   🚀 [CUDA ENGINE] NVIDIA CUDA Aktif: NVIDIA GeForce RTX 4090 (VRAM: 24564 MB, Driver: 550.54)
   ⚡ [CUDA SOLVER] Ditemukan accelerator binary: kangaroo-cuda
   ```

---

## 🧪 Pengujian Unit Test

Engine telah dilengkapi dengan rangkaian pengujian komprehensif, termasuk pengujian known-answer terhadap **Bitcoin Puzzle #20**:

```bash
cargo test kangaroo_tests -- --nocapture
```

---

## 🔒 Keamanan & Perlindungan

- **Terkunci Permanen**: Alamat tujuan sweep dikonfigurasi melalui `.env` server dan terkunci permanen di memori backend, mencegah manipulasi payload dari browser.
- **Penyimpanan Lokal**: Seluruh private key yang ditemukan disimpan secara lokal di basis data PostgreSQL internal dan dicadangkan ke log file lokal terisolasi.
- **Kerahasiaan Kredensial**: File `.env`, kredensial RPC, dan bot token secara ketat dikecualikan dari Git melalui `.gitignore`.

---

## 📄 Lisensi

Proyek ini dirilis di bawah lisensi **MIT**. Silakan lihat file [LICENSE](LICENSE) untuk informasi lebih lanjut.
