#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — GPU HOSTING AUTO SETUP (BACKEND ONLY)
#   Target: GPU Cloud Instances (Vast.ai, RunPod, Lambda Labs, AWS EC2, RTX 6000 Ada/S, etc.)
#   Fungsi:
#     - Deploy backend Rust & PostgreSQL di GPU hosting
#     - Import database lokal (puzzles & 240k+ scanned windows)
#     - Frontend di-hosting terpisah di Vercel (bukan di GPU host)
# ==============================================================================
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║   🚀 BITCOIN PUZZLE HUNTER — BACKEND GPU HOSTING SETUP             ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"
echo -e "📁 Direktori Proyek: ${GREEN}$PROJECT_DIR${NC}\n"

# ── 1. Deteksi GPU NVIDIA ───────────────────────────────────────────────────
echo -e "${CYAN}━━━ [1/6] Memeriksa Hardware GPU NVIDIA ━━━${NC}"
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
echo -e "\n${CYAN}━━━ [2/6] Memeriksa & Menginstall Dependensi Sistem ━━━${NC}"
if command -v apt-get &>/dev/null; then
    sudo apt-get update -qq
    sudo apt-get install -y -qq \
        build-essential libgmp-dev libssl-dev pkg-config git curl wget \
        docker.io docker-compose-plugin
elif command -v yum &>/dev/null; then
    sudo yum install -y gcc gcc-c++ gmp-devel openssl-devel pkgconfig git curl wget
fi
echo -e "${GREEN}✅ Dependensi sistem siap.${NC}"

# ── 3. Install Rust Toolchain ───────────────────────────────────────────────
echo -e "\n${CYAN}━━━ [3/6] Memeriksa Rust Compiler ━━━${NC}"
if ! command -v cargo &>/dev/null; then
    echo "🦀 Mengunduh dan memasang Rust toolchain (stable)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
else
    echo -e "${GREEN}✅ Rust sudah terpasang: $(rustc --version)${NC}"
fi
source "$HOME/.cargo/env" 2>/dev/null || true

# ── 4. Konfigurasi Environment (.env) ───────────────────────────────────────
echo -e "\n${CYAN}━━━ [4/6] Konfigurasi Environment (.env) ━━━${NC}"
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
echo -e "\n${CYAN}━━━ [5/6] Setup PostgreSQL & Migrasi Data ━━━${NC}"
if command -v docker &>/dev/null; then
    docker compose up -d postgres
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
        echo -e "${YELLOW}📦 Ditemukan file backup lokal (data/puzzle_hunt_backup.sql)!${NC}"
        echo -e "${YELLOW}   Mengimpor seluruh data puzzles dan scanned windows...${NC}"
        bash import_db.sh "data/puzzle_hunt_backup.sql"
    else
        echo -e "${YELLOW}ℹ️  Tidak ada data/puzzle_hunt_backup.sql. Menjalankan skema dasar dan seed awal...${NC}"
        if [ -f "data/seed_unsolved.sql" ]; then
            docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt < "data/seed_unsolved.sql" > /dev/null 2>&1 || true
        fi
        echo -e "${GREEN}✅ PostgreSQL siap dengan dataset default.${NC}"
    fi
else
    echo -e "${YELLOW}⚠️ Docker tidak terdeteksi. Pastikan PostgreSQL lokal berjalan di port 5432.${NC}"
fi

# ── 6. Kompilasi Release Rust Backend ───────────────────────────────────────
echo -e "\n${CYAN}━━━ [6/6] Kompilasi Rust Backend (Release Binary) ━━━${NC}"
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
echo -e "${BOLD}📌 Langkah Menjalankan Backend:${NC}"
echo "   bash start_gpu.sh"
echo "   (atau jalankan di background dengan: screen -S puzzle-backend bash start_gpu.sh)"
echo ""
echo -e "${BOLD}🌐 Menghubungkan Frontend di Vercel:${NC}"
echo "   1. Deploy folder 'frontend' ke Vercel"
echo "   2. Di pengaturan Environment Variables Vercel, tambahkan:"
echo -e "      ${YELLOW}VITE_API_URL=http://$PUBLIC_IP:$PORT${NC}"
echo ""
