#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — ALL-IN-ONE GPU HOSTING SETUP
#   Target: GPU Cloud Instances (Vast.ai, RunPod, Lambda Labs, AWS EC2, RTX 5090/4090/6000 Ada)
#   Fungsi:
#     - [1/7] Deteksi hardware GPU NVIDIA & arsitektur CUDA
#     - [2/7] Pasang dependensi sistem Linux (build tools, clang, docker, nvtop)
#     - [3/7] Pasang Rust compiler terbaru (rustup stable)
#     - [4/7] Setup konfigurasi .env otomatis (HUNT_MODE=gpu, stratified, accumulator)
#     - [5/7] Setup PostgreSQL (Docker container / native) & migrasi schema awal
#     - [6/7] Kompilasi cuBitCrack GPU Accelerator (Blackwell sm_120 / Ada sm_89)
#     - [7/7] Kompilasi Rust Backend Engine (Release mode LTO)
# ==============================================================================
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║   🚀 BITCOIN PUZZLE HUNTER — ALL-IN-ONE GPU HOSTING SETUP           ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"
echo -e "📁 Direktori Proyek: ${GREEN}$PROJECT_DIR${NC}\n"

# ── 1. Deteksi GPU NVIDIA ───────────────────────────────────────────────────
echo -e "${CYAN}━━━ [1/7] Memeriksa Hardware GPU NVIDIA ━━━${NC}"
if command -v nvidia-smi &>/dev/null; then
    GPU_NAME=$(nvidia-smi --query-gpu=name --format=csv,noheader | head -1)
    GPU_MEM=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader | head -1)
    DRIVER_VER=$(nvidia-smi --query-gpu=driver_version --format=csv,noheader | head -1)
    echo -e "   GPU Terdeteksi : ${GREEN}$GPU_NAME${NC}"
    echo -e "   Kapasitas VRAM : ${GREEN}$GPU_MEM${NC}"
    echo -e "   Driver Version : ${GREEN}$DRIVER_VER${NC}"
else
    echo -e "${YELLOW}⚠️  'nvidia-smi' tidak ditemukan. Pastikan GPU driver sudah terpasang jika ingin akselerasi hardware.${NC}"
fi

# ── 2. Install Dependensi Sistem Linux ──────────────────────────────────────
echo -e "\n${CYAN}━━━ [2/7] Memeriksa & Menginstall Dependensi Sistem ━━━${NC}"
SUDO=""
if command -v sudo &>/dev/null; then
    SUDO="sudo"
fi

if command -v apt-get &>/dev/null; then
    $SUDO apt-get update -qq
    $SUDO apt-get install -y -qq \
        build-essential cmake pkg-config libssl-dev libgmp-dev libclang-dev clang \
        git curl wget screen tmux htop nvtop psmisc python3 python3-pip \
        postgresql postgresql-contrib docker.io docker-compose 2>/dev/null || \
    $SUDO apt-get install -y -qq \
        build-essential cmake pkg-config libssl-dev libgmp-dev libclang-dev clang \
        git curl wget screen tmux htop psmisc python3 python3-pip
elif command -v yum &>/dev/null; then
    $SUDO yum install -y gcc gcc-c++ cmake gmp-devel openssl-devel pkgconfig git curl wget screen tmux htop python3
fi
echo -e "${GREEN}✅ Dependensi sistem siap.${NC}"

# ── 3. Install Rust Toolchain ───────────────────────────────────────────────
echo -e "\n${CYAN}━━━ [3/7] Memeriksa Rust Compiler ━━━${NC}"
if ! command -v cargo &>/dev/null; then
    echo "🦀 Mengunduh dan memasang Rust toolchain (stable)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env" 2>/dev/null || true
    [ -f "/root/.cargo/env" ] && source "/root/.cargo/env" 2>/dev/null || true
else
    echo -e "${GREEN}✅ Rust sudah terpasang: $(rustc --version)${NC}"
fi
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env" 2>/dev/null || true
[ -f "/root/.cargo/env" ] && source "/root/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:/root/.cargo/bin:$PATH"

# ── 4. Konfigurasi Environment (.env) ───────────────────────────────────────
echo -e "\n${CYAN}━━━ [4/7] Konfigurasi Environment (.env) ━━━${NC}"
if [ ! -f ".env" ]; then
    cp .env.example .env
    echo "📋 Berkas .env dibuat dari .env.example"
fi

# Pastikan HUNT_MODE=gpu dan HOST=0.0.0.0
sed -i 's/^HUNT_MODE=.*/HUNT_MODE=gpu/' .env
sed -i 's/^HOST=.*/HOST=0.0.0.0/' .env

# Generate secret token jika masih memakai nilai default
if grep -q "super_secret_puzzle_hunt_auth_key_2026" .env; then
    RAND_SECRET=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')
    sed -i "s/^AUTH_SECRET=.*/AUTH_SECRET=$RAND_SECRET/" .env
    echo -e "${GREEN}✅ AUTH_SECRET acak berhasil dibuat untuk keamanan API.${NC}"
fi
echo -e "${GREEN}✅ .env berhasil dikonfigurasi untuk GPU hosting (HUNT_MODE=gpu).${NC}"

# ── 5. Setup Database PostgreSQL & Migrasi Data ─────────────────────────────
echo -e "\n${CYAN}━━━ [5/7] Setup PostgreSQL & Migrasi Data ━━━${NC}"
if command -v docker &>/dev/null && docker ps &>/dev/null; then
    docker compose up -d postgres 2>/dev/null || docker run -d --name puzzle_postgres \
        --restart always \
        -e POSTGRES_USER=postgres \
        -e POSTGRES_PASSWORD=postgrespassword \
        -e POSTGRES_DB=puzzle_hunt \
        -p 5432:5432 \
        -v puzzle_postgres_data:/var/lib/postgresql/data \
        postgres:16-alpine 2>/dev/null || true

    echo "⏳ Menunggu container PostgreSQL siap..."
    sleep 4

    MAX_RETRIES=15
    COUNT=0
    until docker exec puzzle_postgres pg_isready -U postgres -d puzzle_hunt &>/dev/null || [ $COUNT -ge $MAX_RETRIES ]; do
        sleep 2
        COUNT=$((COUNT+1))
    done

    # Cek apakah ada file backup lokal yang dibawa user
    if [ -f "data/puzzle_hunt_backup.sql" ]; then
        echo -e "${YELLOW}📦 Ditemukan file backup lokal (data/puzzle_hunt_backup.sql)! Mengimpor data...${NC}"
        bash import_db.sh "data/puzzle_hunt_backup.sql" 2>/dev/null || true
    elif [ -f "data/seed_unsolved.sql" ]; then
        docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt < "data/seed_unsolved.sql" > /dev/null 2>&1 || true
        echo -e "${GREEN}✅ PostgreSQL (Docker) siap dengan dataset default.${NC}"
    fi
else
    echo -e "${YELLOW}ℹ️ Menyiapkan PostgreSQL Native di host...${NC}"
    $SUDO service postgresql start || true
    $SUDO -u postgres psql -c "ALTER USER postgres PASSWORD 'postgrespassword';" 2>/dev/null || true
    $SUDO -u postgres createdb -O postgres puzzle_hunt 2>/dev/null || true
    if [ -f "data/seed_unsolved.sql" ]; then
        $SUDO -u postgres psql -d puzzle_hunt < "data/seed_unsolved.sql" > /dev/null 2>&1 || true
    fi
    echo -e "${GREEN}✅ PostgreSQL (Native) siap & running di port 5432.${NC}"
fi

# ── 6. Kompilasi cuBitCrack GPU Accelerator ─────────────────────────────────
echo -e "\n${CYAN}━━━ [6/7] Memeriksa & Mengompilasi cuBitCrack GPU Solver ━━━${NC}"
if [ -f "build_cubitcrack.sh" ] && command -v nvcc &>/dev/null; then
    if [ ! -f "bin/cuBitCrack" ]; then
        echo "⚡ Mengompilasi cuBitCrack untuk arsitektur GPU server..."
        bash build_cubitcrack.sh || echo -e "${YELLOW}⚠️ Kompilasi cuBitCrack dilewati / gagal, solver native tetap aktif.${NC}"
    else
        echo -e "${GREEN}✅ Binary cuBitCrack sudah tersedia di bin/cuBitCrack.${NC}"
    fi
else
    echo -e "${YELLOW}ℹ️ CUDA compiler (nvcc) belum aktif atau build_cubitcrack.sh tidak ditemukan. Menggunakan engine solver bawaan.${NC}"
fi

# ── 7. Kompilasi Release Rust Backend ───────────────────────────────────────
echo -e "\n${CYAN}━━━ [7/7] Kompilasi Rust Backend (Release Binary) ━━━${NC}"
echo "🦀 Mengompilasi puzzle-hunt release mode..."
cargo build --release --bin puzzle-hunt
echo -e "${GREEN}✅ Binary backend berhasil dibuat: target/release/puzzle-hunt${NC}"

# ── Summary & Next Steps ────────────────────────────────────────────────────
PUBLIC_IP=$(curl -s https://api.ipify.org || hostname -I | awk '{print $1}')
PORT=$(grep "^PORT=" .env | cut -d= -f2 | tr -d '[:space:]')
PORT=${PORT:-3001}

echo ""
echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                  🎉 SETUP BACKEND GPU SELESAI!                       ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo -e "📡 Backend API Endpoint : ${GREEN}http://$PUBLIC_IP:$PORT${NC}"
echo -e "🔌 WebSocket Endpoint   : ${GREEN}ws://$PUBLIC_IP:$PORT/api/ws${NC}"
echo ""
echo -e "${BOLD}📌 Langkah Menjalankan Sistem:${NC}"
echo "   1. Jalankan Cloudflare Tunnel (Token):"
echo "      nohup cloudflared tunnel run --token <TOKEN_ANDA> > logs/cloudflared.log 2>&1 &"
echo "   2. Jalankan Auto-Backup Filebase Daemon (pukul 22.00 WIB):"
echo "      python3 auto_backup_filebase.py --start"
echo "   3. Jalankan Hunter Engine (Daemon Background):"
echo "      bash start_gpu.sh"
echo "   4. Pantau Log & Suhu:"
echo "      tail -f logs/backend.log     # Pantau log perburuan"
echo "      nvtop                        # Pantau utilisasi GPU"
echo ""
