# 🚀 Panduan Eksekusi Setup Server GPU Cloud Baru (Vast.ai / RunPod)

Panduan praktis dan urutan perintah terminal yang sudah teruji saat menyewa server GPU baru (Dual RTX 5090 / RTX 4090) dari kosongan hingga sistem perburuan aktif penuh dengan kecepatan **9.44+ Miliar keys/detik**.

---

## 📋 Urutan Langkah Eksekusi (Copy-Paste Ready)

### 1. Clone Repository & Setup Lingkungan Dasar
Mengunduh kode proyek dan menjalankan inisialisasi awal server (memasang paket dependensi Ubuntu, compiler Rust, Docker/PostgreSQL, dan konfigurasi `.env`).

```bash
cd /workspace
git clone https://github.com/boomsatu/puzzle-hunt.git
cd puzzle-hunt
bash setup_gpu.sh
```
> **Keterangan:**
> - `setup_gpu.sh` otomatis mendeteksi spesifikasi GPU, memasang dependensi sistem, memasang `rustup` & `cargo`, menyalakan database PostgreSQL, dan mengompilasi binary release backend.

---

### 2. Kompilasi Solver GPU Hardware (cuBitCrack & Kangaroo)
Kompilasi engine komputasi CUDA yang disesuaikan khusus dengan arsitektur GPU server (Blackwell `sm_120` / Ada `sm_89` / Ampere `sm_86`).

```bash
# Kompilasi cuBitCrack GPU solver (Wajib untuk Puzzle #71 / Address mode tanpa pubkey)
bash build_cubitcrack.sh

# Kompilasi JeanLucPons Kangaroo CUDA solver (Untuk puzzle dengan public key yang ingin diproses di GPU)
bash build_solver.sh
```
> **Keterangan:**
> - Binary hasil kompilasi akan tersimpan di direktori `bin/cuBitCrack` dan `bin/kangaroo`.
> - Untuk mode CPU (seperti Puzzle #140 dengan 8 core CPU), solver sudah *built-in* otomatis di dalam backend Rust tanpa perlu binary eksternal.

---

### 3. Import Database & Riwayat Scanned Windows dari Filebase IPFS
Mengunduh dan merestore cadangan database terakhir langsung dari jaringan Filebase IPFS ke PostgreSQL server baru agar riwayat pemindaian tidak mengulang dari nol (*anti-duplikasi permanen*).

```bash
bash import_db.sh "https://spotty-brown-marten.myfilebase.com/ipfs/QmSGeAH4TEMRzAcJsVmHcofp5Rbz8VakWkn1i6A2awoEnn"
```
> **Keterangan:**
> - Script akan mengunduh file `.sql.gz`, mengekstrak, mengimpor ke PostgreSQL, dan menampilkan verifikasi jumlah data puzzle serta riwayat *scanned windows* yang berhasil diisi.

---

### 4. Aktifkan Optimasi Kinerja GPU (Persistence Mode & NVIDIA MPS)
**Langkah Kritis untuk Speed Maksimal:** Menghilangkan *overhead context switching* saat menjalankan banyak worker simultan di GPU.

```bash
# 1. Kunci driver GPU agar selalu siaga dan clock speed tidak drop
nvidia-smi -pm 1

# 2. Nyalakan NVIDIA Multi-Process Service (MPS) Daemon untuk Dual GPU
export CUDA_VISIBLE_DEVICES=0,1
nvidia-cuda-mps-control -d
```
> **Keterangan:**
> - `nvidia-smi -pm 1`: Mengaktifkan *Persistence Mode* agar driver NVIDIA tetap resident di memori GPU (mencegah latency inisialisasi PCIe di setiap pergantian ronde).
> - `nvidia-cuda-mps-control -d`: Mengaktifkan **NVIDIA MPS**. Tanpa MPS, 32 worker akan saling berebut antrean kernel GPU. Dengan MPS, seluruh 32 worker digabung menjadi 1 hardware context tunggal langsung di core GPU RTX 5090 (speed stabil di **5.14+ B k/s**).

---

### 5. Hubungkan Cloudflare Zero Trust Tunnel (Akses HTTPS / WSS)
Membuka jalur aman dari port 3001 server ke domain publik tanpa perlu port-forwarding / membuka firewall NAT Vast.ai.

```bash
mkdir -p logs
service cloudflared start || nohup cloudflared tunnel run --token <token> > logs/cloudflared.log 2>&1 &
```
> **Keterangan:**
> - Mengarahkan lalu lintas dari `https://gpu-engine-one.bariscode.my.id` langsung ke `localhost:3001`.
> - Mendukung enkripsi HTTPS dan WebSocket aman (`wss://`) sehingga dashboard Vercel tidak mengalami kendala *Mixed Content*.

---

### 6. Aktifkan Auto-Backup Filebase S3 Daemon (Setiap 3 Jam + Heartbeat Telegram Tiap 30 Menit)
Menjalankan daemon otomatis di background untuk mengekspor database secara rutin ke Filebase Cloud Storage setiap 3 jam sekali (00, 03, 06, 09, 12, 15, 18, 21 WIB), sekaligus mengirimkan notifikasi detak jantung (*Heartbeat*) ke Telegram setiap 30 menit.

```bash
# Jalankan daemon background (Backup otomatis tiap 3 Jam + Heartbeat Telegram 30 Menit)
python3 auto_backup_filebase.py --start

# (Opsional) Uji coba kirim Heartbeat ke Telegram sekarang juga:
python3 auto_backup_filebase.py --heartbeat
```
> **Keterangan:**
> - Berjalan sebagai background daemon mandiri (Unix double-forking — aman tutup terminal SSH).
> - Otomatis mengekspor database dan mengompresi menjadi `.sql.gz` setiap **3 jam sekali** (00:00, 03:00, 06:00, 09:00, 12:00, 15:00, 18:00, 21:00 WIB).
> - **Heartbeat Telegram:** Mengirim status runtime hunter, kesehatan PostgreSQL, dan utilisasi GPU setiap 30 menit via Cloudflare Worker Proxy yang ada di `.env`.
> - Untuk memeriksa status daemon: `python3 auto_backup_filebase.py --status`.

---

### 7. Jalankan Engine Perburuan (Background Daemon)
Menyalakan PostgreSQL, membersihkan proses lama, dan menjalankan backend perburuan di background.

```bash
bash start_gpu.sh
```
> **Keterangan:**
> - Proses backend berjalan via `nohup` dan menyimpan nomor PID ke `.backend.pid`.
> - **Aman menutup terminal SSH atau mematikan laptop tanpa menghentikan perburuan**.

---

## 📊 Cheatsheet Pemantauan & Operasional Harian

| Tugas | Perintah Terminal |
|---|---|
| **Pantau Log Perburuan Real-time** | `tail -f logs/backend.log` |
| **Pantau Suhu & Utilisasi GPU (NVIDIA)** | `nvtop` *(atau `watch -n 1 nvidia-smi`)* |
| **Pantau Beban CPU & RAM** | `htop` |
| **Hentikan Sistem dengan Aman (Graceful Pause)** | `bash stop.sh` |
| **Cek Status Cloudflare Tunnel** | `ps aux \| grep cloudflared` |
| **Cek Status Daemon Backup Filebase** | `python3 auto_backup_filebase.py --status` |
| **Jalankan Backup Database Manual Instan** | `python3 auto_backup_filebase.py --now` |

---

## 🎯 Golden Setup di Dashboard Web (Sweet Spot 9.44 B keys/s)

Setelah backend aktif, buka dashboard Anda di browser dan atur perburuan:

1. **Puzzle #71 (Mode Address / Tanpa Pubkey)**:
   - Klik **Start Hunt** $\rightarrow$ Pilih Mode **GPU Acceleration**.
   - Workers: `32`
   - Window: `32-bit`
   - *Throughput:* **~5.14 Miliar keys/detik**
2. **Puzzle #140 (Mode Kangaroo / Exposed Pubkey)**:
   - Klik **Start Hunt** $\rightarrow$ Pilih Mode **CPU High-Performance**.
   - Workers: `8`
   - Window: `30-bit`
   - *Throughput:* **~4.30 Miliar keys/detik**
3. **Total Kecepatan Simultan:** **🔥 9.44 Miliar keys/detik stabil tanpa saturasi bus PCIe!**

```
> **Keterangan:**
> - Proses backend berjalan via `nohup` dan menyimpan nomor PID ke `.backend.pid`.
> - **Aman menutup terminal SSH atau mematikan laptop tanpa menghentikan perburuan**.

---

## 📊 Cheatsheet Pemantauan & Operasional Harian

| Tugas | Perintah Terminal |
|---|---|
| **Pantau Log Perburuan Real-time** | `tail -f logs/backend.log` |
| **Pantau Suhu & Utilisasi GPU (NVIDIA)** | `nvtop` *(atau `watch -n 1 nvidia-smi`)* |
| **Pantau Beban CPU & RAM** | `htop` |
| **Hentikan Sistem dengan Aman (Graceful Pause)** | `bash stop.sh` |
| **Cek Status Cloudflare Tunnel** | `ps aux \| grep cloudflared` |
| **Cek Status Daemon Backup Filebase** | `python3 auto_backup_filebase.py --status` |
| **Jalankan Backup Database Manual Instan** | `python3 auto_backup_filebase.py --now` |

---

## 🎯 Golden Setup di Dashboard Web (Sweet Spot 9.44 B keys/s)

Setelah backend aktif, buka dashboard Anda di browser dan atur perburuan:

1. **Puzzle #71 (Mode Address / Tanpa Pubkey)**:
   - Klik **Start Hunt** $\rightarrow$ Pilih Mode **GPU Acceleration**.
   - Workers: `32`
   - Window: `32-bit`
   - *Throughput:* **~5.14 Miliar keys/detik**
2. **Puzzle #140 (Mode Kangaroo / Exposed Pubkey)**:
   - Klik **Start Hunt** $\rightarrow$ Pilih Mode **CPU High-Performance**.
   - Workers: `8`
   - Window: `30-bit`
   - *Throughput:* **~4.30 Miliar keys/detik**
3. **Total Kecepatan Simultan:** **🔥 9.44 Miliar keys/detik stabil tanpa saturasi bus PCIe!**

## Jalankan auto backup database
python3 auto_backup_filebase.py --start

## Cek log
tail -f logs/backend.log
