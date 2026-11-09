# 🦀 Bitcoin Puzzle Hunter

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16-blue.svg?style=flat-square&logo=postgresql)](https://www.postgresql.org/)
[![React](https://img.shields.io/badge/React-18-61DAFB.svg?style=flat-square&logo=react)](https://react.dev/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-3-38B2AC.svg?style=flat-square&logo=tailwind-css)](https://tailwindcss.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)

**Bitcoin Puzzle Hunter** adalah platform berkinerja tinggi yang dirancang khusus untuk memecahkan [Bitcoin Challenge Transaction Puzzles](https://blockchain.com). Dibangun dari awal menggunakan **Rust** murni dengan operasi titik kurva eliptik (*secp256k1*), terintegrasi dengan basis data **PostgreSQL** untuk pelacakan anti-duplikasi permanen, serta **Web UI Dashboard** real-time berbasis React & WebSockets.

---

## ⚡ Fitur Utama

- 🦘 **Dual-Engine Hunting**:
  - **Pollard's Kangaroo Engine $O(\sqrt{N})$**: Untuk puzzle yang memiliki Public Key terbuka (#140, #145, #150, #155, #160). Mampu memeriksa window berukuran **1 Miliar kunci hanya dalam ~1,5 detik** per thread.
  - **Address Hash160 Scanner Engine**: Untuk puzzle yang public key-nya belum terekspos (#66, #67, dll). Melakukan pemindaian sekuensial rapat ($P_{i+1} = P_i + G$) berbasis hash160.
- ⚡ **Dynamic Multi-Worker Scaling**:
  - Menyesuaikan kapasitas CPU secara otomatis: jika hanya menjalankan 1 atau 2 puzzle, sistem secara otomatis mengalokasikan hingga **7–8 worker threads paralel per puzzle**.
  - Mencapai kecepatan gabungan **6 hingga 7+ Miliar kunci per detik** pada CPU modern 16-thread.
- 🛡️ **Anti-Duplikasi Permanen (PostgreSQL)**:
  - Setiap rentang window (`[w_start, w_end]`) yang telah diperiksa dicatat ke PostgreSQL dengan index b-tree.
  - Pemilihan rentang acak (*Random Jump Monte Carlo*) menjamin **100% daya komputasi mengeksplorasi ruang baru**, tanpa membuang siklus CPU pada window yang pernah discan.
- 🚀 **Auto-Transfer Instan (Anti Front-Running)**:
  - Deteksi temuan kunci privat secara instan memicu penandatanganan transaksi Bitcoin mentah (*raw transaction*).
  - Menggunakan **High Priority Miner Fee** (2x `estimatesmartfee(1)` atau minimal 35 sat/vB) untuk memastikan transaksi diproses miner di blok pertama.
  - Disiarkan langsung melalui node Bitcoin JSON-RPC tercepat (`bitcoin-rpc.publicnode.com`).
- 📲 **Notifikasi Real-Time Telegram**:
  - Peringatan instan saat private key ditemukan lengkap dengan alamat, saldo, nilai WIF terenkripsi, dan tautan penjelajah transaksi on-chain [Blockchain.com](https://www.blockchain.com).
- 🖥️ **Dashboard Web Interaktif**:
  - Monitor live speed (k/s), status koneksi WebSocket, grafik progress window, dan riwayat penjelajahan.
  - Fitur tambah puzzle dinamis dengan pendeteksi otomatis riwayat public key on-chain.

---

## 🏗️ Arsitektur Sistem

```
┌────────────────────────────────────────────────────────┐
│             React + Vite Web Dashboard                │
│             (Port 5173 / WebSocket WSS)               │
└──────────────────────────┬─────────────────────────────┘
                           │ REST API & WebSocket (/api/ws)
┌──────────────────────────▼─────────────────────────────┐
│             Axum Web Server (Port 3001)                │
└──────────────────────────┬─────────────────────────────┘
                           │
       ┌───────────────────┼───────────────────┐
       ▼                   ▼                   ▼
┌──────────────┐   ┌──────────────┐   ┌─────────────────┐
│ Dynamic Hunt │   │  PostgreSQL  │   │  Auto-Transfer  │
│   Manager    │   │  Database    │   │     Engine      │
│ (Rayon/Tokio)│   │ (Anti-Dup)   │   │  (RPC + Miner)  │
└──────┬───────┘   └──────────────┘   └────────┬────────┘
       │                                       │
       ├───────────────────┬───────────────────┤
       ▼                   ▼                   ▼
┌──────────────┐   ┌──────────────┐   ┌─────────────────┐
│ Pollard's    │   │ Address      │   │ Telegram Alert  │
│ Kangaroo     │   │ Hash160      │   │ Notifier        │
│ O(sqrt(N))   │   │ Scanner      │   │                 │
└──────────────┘   └──────────────┘   └─────────────────┘
```

---

## 🚀 Panduan Memulai Cepat

### 1. Prasyarat
- [Rust Toolchain](https://rustup.rs/) (v1.75+ disarankan)
- [Docker & Docker Compose](https://www.docker.com/) (untuk PostgreSQL)
- [Node.js](https://nodejs.org/) (v18+ untuk dashboard frontend)

### 2. Kloning Repositori
```bash
git clone https://github.com/lumina-blockchain/puzzle-hunt.git
cd puzzle-hunt
```

### 3. Konfigurasi Environment
Salin file template environment dan sesuaikan konfigurasi bila perlu:
```bash
copy .env.example .env
```

### 4. Jalankan Database PostgreSQL
Gunakan Docker Compose untuk menyalakan PostgreSQL:
```bash
docker compose up -d
```

### 5. Kompilasi & Jalankan Backend Rust
```bash
# Kompilasi rilis optimal (Level 3 optimization + LTO)
cargo build --release

# Jalankan server
./target/release/puzzle-hunt
```
*Backend akan berjalan di `http://localhost:3001`.*

### 6. Jalankan Frontend Dashboard
Buka terminal baru:
```bash
cd frontend
npm install
npm run dev
```
*Akses dashboard melalui browser di `http://localhost:5173`.*

---

## 🧪 Menjalankan Pengujian Unit Test

Engine telah dilengkapi dengan 9 unit test komprehensif, termasuk pengujian known-answer terhadap **Bitcoin Puzzle #20**:

```bash
cargo test kangaroo_tests -- --nocapture
```

---

## 🔒 Keamanan & Perlindungan Privasi

- Alamat tujuan penarikan otomatis dikonfigurasi melalui `.env` dan terkunci secara permanen di backend untuk mencegah manipulasi dari sisi antarmuka client.
- Seluruh private key yang ditemukan disimpan secara lokal di database PostgreSQL internal dan file backup lokal terisolasi.
- File konfigurasi `.env` dan token rahasia diabaikan secara ketat oleh `.gitignore`.

---

## 📄 Lisensi

Proyek ini dilisensikan di bawah lisensi **MIT**. Silakan lihat file [LICENSE](LICENSE) untuk rincian lengkap.
