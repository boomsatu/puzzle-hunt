#!/bin/bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER â€” GPU INSTANCE QUICK START (BACKEND ONLY)
#   Usage: bash start_gpu.sh
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
echo "â•”â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•—"
echo "â•‘   ðŸš€  PUZZLE HUNTER â€” START BACKEND GPU HOSTING                      â•‘"
echo "â•šâ•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•"
echo -e "${NC}"

# 1. Check GPU Hardware
GPU_NAME=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1 || echo "NVIDIA Driver not loaded")
GPU_MEM=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader 2>/dev/null | head -1 || echo "N/A")
echo -e "GPU   : ${GREEN}$GPU_NAME ($GPU_MEM)${NC}"

# 2. Start PostgreSQL
echo -e "\n${CYAN}[1/3] Menyalakan PostgreSQL...${NC}"
if command -v docker &>/dev/null && docker ps &>/dev/null; then
    docker compose up -d postgres
    sleep 3
    if docker exec puzzle_postgres pg_isready -U postgres -d puzzle_hunt &>/dev/null; then
        echo -e "${GREEN}âœ… PostgreSQL (Docker) siap & running.${NC}"
    else
        echo -e "${RED}âŒ PostgreSQL belum siap. Cek log: docker logs puzzle_postgres${NC}"
        exit 1
    fi
else
    service postgresql start 2>/dev/null || true
    echo -e "${GREEN}âœ… PostgreSQL (Native) siap & running.${NC}"
fi

# 3. Check & Build Binary
echo -e "\n${CYAN}[2/3] Memeriksa & Mengompilasi Binary Backend...${NC}"
if command -v cargo &>/dev/null; then
    echo "ðŸ¦€ Mengompilasi versi release terbaru dengan cargo..."
    cargo build --release --bin puzzle-hunt
    echo -e "${GREEN}âœ… Binary siap: target/release/puzzle-hunt${NC}"
elif [ -f "target/release/puzzle-hunt" ]; then
    echo -e "${GREEN}âœ… Menggunakan binary rilis yang sudah ada: target/release/puzzle-hunt${NC}"
else
    echo -e "${RED}âŒ Error: 'cargo' tidak ditemukan dan target/release/puzzle-hunt belum ada.${NC}"
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
echo -e "${YELLOW}ðŸ’¡ Tip: Sambungkan Vercel dengan menyetel VITE_API_URL=http://$PUBLIC_IP:$PORT${NC}"

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
    echo -e "${GREEN}âœ… Backend berjalan aman di BACKGROUND DAEMON (PID: $PID).${NC}"
    echo -e "${GREEN}ðŸ”’ Anda AMAN untuk menutup terminal SSH atau mematikan laptop sekarang!${NC}"
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
    echo -e "${YELLOW}âš ï¸  PERHATIAN: Berjalan di FOREGROUND (Ctrl+C untuk pause/stop).${NC}"
    echo "â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”â”"
    RUST_LOG=info ./target/release/puzzle-hunt
fi
