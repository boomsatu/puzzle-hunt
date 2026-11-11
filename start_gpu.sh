#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — GPU INSTANCE QUICK START (BACKEND ONLY)
#   Usage: bash start_gpu.sh
# ==============================================================================
set -e

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
echo -e "\n${CYAN}[1/3] Menyalakan PostgreSQL via Docker...${NC}"
docker compose up -d postgres
sleep 3
if docker exec puzzle_postgres pg_isready -U postgres -d puzzle_hunt &>/dev/null; then
    echo -e "${GREEN}✅ PostgreSQL siap & running.${NC}"
else
    echo -e "${RED}❌ PostgreSQL belum siap. Cek log: docker logs puzzle_postgres${NC}"
    exit 1
fi

# 3. Check Binary
echo -e "\n${CYAN}[2/3] Memeriksa Binary Backend...${NC}"
if [ ! -f "target/release/puzzle-hunt" ]; then
    echo "🦀 Binary tidak ditemukan, mengompilasi release..."
    cargo build --release --bin puzzle-hunt
fi
echo -e "${GREEN}✅ Binary siap: target/release/puzzle-hunt${NC}"

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
echo -e "${YELLOW}📝 Log output real-time (Ctrl+C untuk pause/stop):${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

RUST_LOG=info ./target/release/puzzle-hunt
