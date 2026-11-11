# 🦀 Bitcoin Puzzle Hunter

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16-blue.svg?style=flat-square&logo=postgresql)](https://www.postgresql.org/)
[![NVIDIA CUDA](https://img.shields.io/badge/NVIDIA_CUDA-Ready-76B900.svg?style=flat-square&logo=nvidia)](https://developer.nvidia.com/cuda-zone)
[![React](https://img.shields.io/badge/React-18-61DAFB.svg?style=flat-square&logo=react)](https://react.dev/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-3-38B2AC.svg?style=flat-square&logo=tailwind-css)](https://tailwindcss.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=flat-square)](https://opensource.org/licenses/MIT)

**Bitcoin Puzzle Hunter** is an enterprise-grade, high-performance platform designed specifically to solve the [Bitcoin Challenge Transaction Puzzles](https://blockchain.com). Built from scratch in pure **Rust** with native elliptic curve operations (*secp256k1*), dynamic multi-thread scaling, **PostgreSQL** database integration for permanent deduplication, multi-device runtime execution (`local`, `vps`, `gpu`), and a real-time interactive **Web UI Dashboard** powered by React and WebSockets.

---

## ⚡ Key Features & Highlights

### 1. ⚙️ Flexible Execution Runtime Modes (`HUNT_MODE`)
Tailor CPU/GPU hardware utilization directly via the `.env` configuration file:
- **`local` Mode (Default - Desktop PC)**:
  - Balanced resource allocation.
  - Automatically **reserves 2 CPU cores** for the operating system, web browser, PostgreSQL, and background tasks (maximum 8 worker threads per puzzle). Ensures your desktop remains smooth and responsive without freezing.
- **`vps` Mode (Headless Linux Cloud Server)**:
  - Specially optimized for cloud VPS instances (Ubuntu/Debian).
  - Reserves **only 1 core** for OS + PostgreSQL + Web API, dedicating all remaining cores (e.g., 7 out of 8 cores, 15 out of 16 cores) to puzzle cracking at maximum throughput.
- **`gpu` / `cuda` Mode (NVIDIA Hardware Acceleration)**:
  - Automatically detects NVIDIA GPU presence and driver version via `nvidia-smi` (GPU name, total VRAM, driver version).
  - Supports external CUDA accelerator solvers (`kangaroo-cuda`, `keyhunt`, `bitcrack`) as well as integrated GPU dispatch pipelines.
  - Features an **automatic fail-safe fallback**: if GPU mode is specified on a host without CUDA hardware, the engine automatically falls back to maximum VPS multi-core mode.
  - Ready for cloud GPU rental platforms such as **Vast.ai, RunPod, AWS EC2 G-Series, Hostinger GPU**, etc.

### 2. ⏸️ Smart Graceful Pause (*Zero Data Loss*)
- Unlike brute-force process termination that risks database corruption and wasted cycles, **Smart Graceful Pause** guarantees **Zero Data Loss**.
- When the user clicks **"Pause Hunter"**, the hunter state transitions to `Pausing` (*"PAUSING... Finishing Current Window"*).
- Active workers complete their in-progress window computations naturally.
- All scanned window ranges and metrics are safely committed to PostgreSQL (`scanned_windows`).
- Once persisted, the state transitions to `Paused` and workers idle cleanly.
- An emergency endpoint (`POST /api/hunt/:number/force_stop`) remains available for immediate instant termination when required.

### 3. 🎉 Real-Time Celebration & Auto-Transfer Timeline
- **Interactive Victory Modal**: Featuring dynamic confetti animations whenever a private key is discovered.
- **Synthesized Audio Fanfare**: Uses the native **Web Audio API** to generate a melodic victory fanfare (*C-E-G-C-E-G-C* arpeggio) directly in the browser—zero external audio files or third-party dependencies required.
- **Live WebSocket Event Pipeline**: The Rust backend streams every milestone of the auto-transfer sweep in real-time:
  1. 🔑 **Key Found & Verified**: Private key discovered, converted to WIF, and validated against target public key / address.
  2. 📝 **Instant Sweep Preparation**: UTXO lookup and raw transaction construction.
  3. ✍️ **Cryptographic Signing**: Secp256k1 digital signature generation.
  4. 📡 **Network Broadcast**: Transaction broadcast via public Bitcoin JSON-RPC nodes (`bitcoin-rpc.publicnode.com`).
  5. ⛏️ **Mempool & Block Confirmation**: Real-time TxID generation with direct links to the Blockchain.com explorer.
- **Simulation Mode**: A *"Simulate Celebration"* button in the puzzle detail view allows testing the entire UI, audio, and timeline pipeline at any time.

### 4. 🦘 High-Performance Dual-Engine Architecture
- **Pollard's Kangaroo Engine $O(\sqrt{N})$**:
  - Specialized for puzzles with exposed uncompressed/compressed Public Keys (#140, #145, #150, #155, #160).
  - Capable of searching a **1 Billion key window in ~1.5 seconds** per thread.
- **Sequential Address Hash160 Scanner Engine**:
  - Designed for puzzles where the public key has never been revealed (#66, #67, etc.).
  - Executes rapid sequential point addition ($P_{i+1} = P_i + G$) with hash160 comparison.

### 5. 🛡️ Permanent Deduplication (PostgreSQL)
- Every scanned window range (`[w_start, w_end]`) is indexed with B-Tree tables in PostgreSQL.
- Pseudo-random Monte Carlo jump selection ensures **100% of compute cycles explore virgin key space**, eliminating duplicate scanning.

### 6. 🚀 Instant Anti Front-Running Auto-Sweep
- Destination payout address is configured in `.env` and locked permanently at the backend level.
- Applies **High Priority Miner Fees** (2x `estimatesmartfee(1)` or minimum 35 sat/vB) to ensure inclusion in the very next mined block, neutralizing front-running mempool bots.

### 7. 📊 Modern, Distraction-Free Web Dashboard
- **Dual Display Modes**: Toggle seamlessly between **Grid View** (visual cards) and **Table View** (compact tabular data), with preference saved to `localStorage`.
- **Smart Filters**: Instant switching between *All Puzzles*, *Pubkey Exposed (Kangaroo $O(\sqrt{N})$)*, and *No Pubkey (Address Mode)*.
- **Isolated Detail View**: Global dashboard metrics remain on the home overview; individual puzzle pages focus exclusively on live hunt progress, multi-thread jump positions (*Core Jump Positions*), audit logs, and historical window data.
- **160-Bit High-Precision Math**: Accurate BigInt calculations without scientific e-notation in percentage and key space metrics.

---

## 🏗️ System Architecture

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

## ⚙️ Environment Configuration (`.env`)

Copy the `.env.example` template into `.env`:

```bash
cp .env.example .env      # Linux / macOS
copy .env.example .env    # Windows PowerShell / CMD
```

Available configuration options:

| Variable | Description | Example / Default |
|---|---|---|
| `DATABASE_URL` | PostgreSQL connection string (Docker or local) | `postgres://postgres:postgrespassword@localhost:5432/puzzle_hunt` |
| `PORT` | REST API & WebSocket server port | `3001` |
| `HOST` | Server host binding interface | `0.0.0.0` |
| `DEFAULT_DESTINATION_ADDRESS` | Permanent Bitcoin address for instant sweep payouts | `bc1q8c0uc752amr7v4amswn5vhes37a6ffy4ral6ek` |
| `BITCOIN_RPC_URL` | Bitcoin JSON-RPC node endpoint for raw transaction broadcast | `https://bitcoin-rpc.publicnode.com` |
| `TELEGRAM_BOT_TOKEN` | Telegram Bot token from [@BotFather](https://t.me/BotFather) (Optional) | `123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ` |
| `TELEGRAM_CHAT_ID` | Telegram chat ID to receive instant alerts (Optional) | `987654321` |
| `HUNT_MODE` | **Execution Mode**: `local` (PC), `vps` (Cloud Server), or `gpu` (NVIDIA CUDA) | `local` |

---

## 🚀 Quick Start Guide

### 1. Prerequisites
- **Rust Toolchain**: v1.75+ ([rustup.rs](https://rustup.rs/))
- **Docker & Docker Compose**: For running the PostgreSQL instance ([docker.com](https://www.docker.com/))
- **Node.js**: v18+ & npm ([nodejs.org](https://nodejs.org/))
- *(Optional)* **NVIDIA GPU Driver & CUDA Toolkit**: Required only if using `HUNT_MODE=gpu`

### 2. Clone the Repository
```bash
git clone https://github.com/boomsatu/puzzle-hunt.git
cd puzzle-hunt
```

### 3. Configure the Environment
```bash
# Windows
copy .env.example .env

# Linux / macOS
cp .env.example .env
```
*Open `.env` and set `HUNT_MODE` to match your hardware (`local`, `vps`, or `gpu`), and configure your destination Bitcoin address.*

### 4. Launch PostgreSQL Database
Start the database service using Docker Compose:
```bash
docker compose up -d postgres
```
*PostgreSQL will be available on port `5432`. Schema migrations and initial puzzle data are applied automatically when the backend launches.*

### 5. Compile & Run Rust Backend
Build with release-level optimizations (`--release`) and LTO:
```bash
cargo build --release

# Run executable
# Windows:
.\target\release\puzzle-hunt.exe

# Linux / macOS:
./target/release/puzzle-hunt
```
*The API and WebSocket server will listen on `http://localhost:3001`.*

### 6. Launch Web Frontend Dashboard
Open a new terminal window in the `frontend` directory:
```bash
cd frontend
npm install
npm run dev
```
*Access the dashboard in your browser at `http://localhost:5173`.*

---

## ☁️ Deployment Guide: Cloud VPS & GPU Hosting

### A. Headless Linux VPS (DigitalOcean / Linode / Contabo / Hetzner)
1. Configure `.env`:
   ```ini
   HUNT_MODE=vps
   HOST=0.0.0.0
   PORT=3001
   ```
2. Start PostgreSQL: `docker compose up -d postgres`
3. Build backend: `cargo build --release`
4. Run as a background service via `systemd` or `tmux`.
5. Build production frontend: `cd frontend && npm run build` (serve with Nginx or any static file server).

### B. Cloud GPU Hosting (Vast.ai / RunPod / AWS EC2 G4/G5)
1. Verify NVIDIA drivers: `nvidia-smi`
2. Configure `.env`:
   ```ini
   HUNT_MODE=gpu
   ```
3. On backend startup, the engine will verify your GPU environment:
   ```text
   🚀 [CUDA ENGINE] NVIDIA CUDA Active: NVIDIA GeForce RTX 4090 (VRAM: 24564 MB, Driver: 550.54)
   ⚡ [CUDA SOLVER] Found accelerator binary: kangaroo-cuda
   ```

---

## 🧪 Running Unit Tests

The engine includes comprehensive test coverage, including known-answer verification against **Bitcoin Puzzle #20**:

```bash
cargo test kangaroo_tests -- --nocapture
```

---

## 🔒 Security & Privacy

- **Immutable Sweep Target**: The payout address is loaded strictly from the server's `.env` and held immutable in backend memory, preventing client-side tampering.
- **Local Key Storage**: All discovered private keys are stored locally within your private PostgreSQL database and backed up to isolated local files.
- **Zero Credential Leaks**: Sensitive files (`.env`, database credentials, and Telegram tokens) are strictly excluded from version control via `.gitignore`.

---

## 📄 License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.
