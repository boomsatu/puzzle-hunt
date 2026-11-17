#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — GPU INSTANCE QUICK START (BACKEND ONLY)
#   Usage: bash start_gpu.sh [--fg]
# ==============================================================================
set -e

# Muat environment Cargo / Rustup agar selalu terbaca di subshell
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env" 2>/dev/null || true
[ -f "/root/.cargo/env" ] && source "/root/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:/root/.cargo/bin:$PATH"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║   🚀  PUZZLE HUNTER — START BACKEND GPU HOSTING                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# 1. Check GPU Hardware
GPU_NAME=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1 || echo "NVIDIA Driver not loaded")
GPU_MEM=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader 2>/dev/null | head -1 || echo "N/A")
echo -e "GPU   : ${GREEN}$GPU_NAME ($GPU_MEM)${NC}"

# 2. Start PostgreSQL
echo -e "\n${CYAN}[1/3] Menyalakan & Memverifikasi PostgreSQL...${NC}"
DB_READY=false

# Cek Docker
if command -v docker &>/dev/null && docker ps &>/dev/null; then
    echo "🐳 Mendeteksi Docker... Menyalakan container database..."
    if ! docker ps -a --format '{{.Names}}' | grep -q "^puzzle_postgres$"; then
        docker run -d --name puzzle_postgres \
            --restart always \
            -e POSTGRES_USER=postgres \
            -e POSTGRES_PASSWORD=postgrespassword \
            -e POSTGRES_DB=puzzle_hunt \
            -p 5432:5432 \
            -v puzzle_postgres_data:/var/lib/postgresql/data \
            postgres:16-alpine 2>/dev/null || docker compose up -d postgres 2>/dev/null || true
    else
        docker start puzzle_postgres 2>/dev/null || docker compose up -d postgres 2>/dev/null || true
    fi

    for i in {1..15}; do
        if docker exec puzzle_postgres pg_isready -U postgres -d puzzle_hunt &>/dev/null; then
            DB_READY=true
            echo -e "${GREEN}✅ PostgreSQL (Docker) siap & running di port 5432.${NC}"
            break
        fi
        sleep 1
    done
fi

# Fallback ke Native PostgreSQL jika Docker tidak siap
if [ "$DB_READY" != "true" ]; then
    echo "🐘 Menyalakan PostgreSQL Native di server..."
    service postgresql start 2>/dev/null || /etc/init.d/postgresql start 2>/dev/null || true
    
    # Inisialisasi user & database jika baru pertama kali
    su - postgres -c "psql -c \"ALTER USER postgres PASSWORD 'postgrespassword';\"" 2>/dev/null || true
    su - postgres -c "createdb -O postgres puzzle_hunt" 2>/dev/null || true
    if [ -f "data/seed_unsolved.sql" ]; then
        su - postgres -c "psql -d puzzle_hunt < data/seed_unsolved.sql" 2>/dev/null || true
    fi

    for i in {1..10}; do
        if pg_isready -h 127.0.0.1 -p 5432 -U postgres &>/dev/null || pg_isready -p 5432 &>/dev/null; then
            DB_READY=true
            echo -e "${GREEN}✅ PostgreSQL (Native) siap & running di port 5432.${NC}"
            break
        fi
        sleep 1
    done
fi

if [ "$DB_READY" != "true" ]; then
    echo -e "${RED}❌ PostgreSQL belum aktif di port 5432!${NC}"
    echo -e "${YELLOW}Coba jalankan secara manual salah satu berikut:${NC}"
    echo "   Docker : docker compose up -d postgres"
    echo "   Native : service postgresql start"
    exit 1
fi

# 3. Check & Build Binary
echo -e "\n${CYAN}[2/3] Memeriksa & Mengompilasi Binary Backend...${NC}"
if command -v cargo &>/dev/null; then
    echo "🦀 Mengompilasi versi release terbaru dengan cargo..."
    cargo build --release --bin puzzle-hunt
    echo -e "${GREEN}✅ Binary siap: target/release/puzzle-hunt${NC}"
elif [ -f "target/release/puzzle-hunt" ]; then
    echo -e "${GREEN}✅ Menggunakan binary rilis yang sudah ada: target/release/puzzle-hunt${NC}"
else
    echo -e "${RED}❌ Error: 'cargo' tidak ditemukan dan target/release/puzzle-hunt belum ada.${NC}"
    exit 1
fi

# 4. Launch Backend
PUBLIC_IP=$(curl -s https://api.ipify.org 2>/dev/null || hostname -I | awk '{print $1}')
PORT=$(grep "^PORT=" .env 2>/dev/null | cut -d= -f2 | tr -d '[:space:]')
PORT=${PORT:-3001}

echo -e "\n${CYAN}[3/3] Menjalankan Puzzle Hunter Engine...${NC}"
echo -e "${GREEN}"
echo "   API Base URL : http://$PUBLIC_IP:$PORT"
echo "   WebSocket    : ws://$PUBLIC_IP:$PORT/api/ws"
echo "   Frontend     : Hosted on Vercel"
echo -e "${NC}"
echo -e "${YELLOW}💡 Tip: Sambungkan Vercel dengan menyetel VITE_API_URL=http://$PUBLIC_IP:$PORT${NC}"

# Default mode: DAEMON BACKGROUND (otomatis aman tutup SSH)
if [ "$1" != "--fg" ] && [ "$1" != "--foreground" ]; then
    mkdir -p logs
    # Bunuh backend lama dan semua proses solver GPU yang tersisa/zombie
    pkill -9 -f kangaroo 2>/dev/null || true
    pkill -9 -f cuBitCrack 2>/dev/null || true
    pkill -9 -f bitcrack 2>/dev/null || true
    pkill -9 -f puzzle-hunt 2>/dev/null || true
    fuser -k $PORT/tcp 2>/dev/null || true
    sleep 1
    nohup ./target/release/puzzle-hunt > logs/backend.log 2>&1 &
    PID=$!
    echo "$PID" > .backend.pid
    echo -e "${GREEN}✅ Backend berjalan aman di BACKGROUND DAEMON (PID: $PID).${NC}"
    echo -e "${GREEN}🔒 Anda AMAN untuk menutup terminal SSH atau mematikan laptop sekarang!${NC}"
    echo -e "${CYAN}   - Pantau log live : tail -f logs/backend.log${NC}"
    echo -e "${CYAN}   - Hentikan        : bash stop.sh (atau kill $PID)${NC}"
else
    # Bersihkan solver GPU sebelum menjalankan foreground
    pkill -9 -f kangaroo 2>/dev/null || true
    pkill -9 -f cuBitCrack 2>/dev/null || true
    pkill -9 -f bitcrack 2>/dev/null || true
    pkill -9 -f puzzle-hunt 2>/dev/null || true
    fuser -k $PORT/tcp 2>/dev/null || true
    sleep 1
    echo -e "${YELLOW}⚠️  PERHATIAN: Berjalan di FOREGROUND (Ctrl+C untuk pause/stop).${NC}"
    echo "──────────────────────────────────────────────────────────────────────"
    RUST_LOG=info ./target/release/puzzle-hunt
fi
