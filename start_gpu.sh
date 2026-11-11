#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — GPU INSTANCE QUICK START
#   Jalankan setelah setup_gpu.sh selesai
#   Usage: bash start_gpu.sh
# ==============================================================================
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║   🚀  PUZZLE HUNTER — GPU START                                     ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# 1. Check GPU
GPU_NAME=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1 || echo "No GPU")
GPU_MEM=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader 2>/dev/null | head -1 || echo "N/A")
echo -e "GPU   : ${GREEN}$GPU_NAME ($GPU_MEM)${NC}"

# 2. Start PostgreSQL
echo -e "\n${CYAN}[1/3] Starting PostgreSQL...${NC}"
docker compose up -d postgres
sleep 3
if docker exec puzzle_postgres pg_isready -U postgres &>/dev/null; then
  echo -e "${GREEN}✅ PostgreSQL OK${NC}"
else
  echo -e "${RED}❌ PostgreSQL gagal start. Cek: docker logs puzzle_postgres${NC}"
  exit 1
fi

# 3. Check binary
echo -e "\n${CYAN}[2/3] Check Backend Binary...${NC}"
if [ ! -f "target/release/puzzle-hunt" ]; then
  echo "🦀 Binary tidak ada, compile dulu..."
  cargo build --release --bin puzzle-hunt
fi
echo -e "${GREEN}✅ Binary ready: target/release/puzzle-hunt${NC}"

# 4. Launch backend
echo -e "\n${CYAN}[3/3] Starting Puzzle Hunter Backend...${NC}"
IP=$(hostname -I | awk '{print $1}')
PORT=$(grep "^PORT=" .env | cut -d= -f2 | tr -d '[:space:]') 
PORT=${PORT:-3001}

echo -e "${GREEN}"
echo "   Dashboard : http://$IP:$PORT"
echo "   API       : http://$IP:$PORT/api"
echo "   WebSocket : ws://$IP:$PORT/ws"
echo -e "${NC}"
echo -e "${YELLOW}📝 Log dimulai di bawah ini (Ctrl+C untuk stop):${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

RUST_LOG=info ./target/release/puzzle-hunt
