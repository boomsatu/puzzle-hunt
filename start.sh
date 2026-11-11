#!/bin/bash
# ══════════════════════════════════════════════════════════════════
# 🔑 Bitcoin Puzzle Hunter — Linux Launcher (VPS / GPU Hosting)
# ══════════════════════════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_DIR"

BACKEND_PID_FILE="$PROJECT_DIR/.backend.pid"
FRONTEND_PID_FILE="$PROJECT_DIR/.frontend.pid"
LOG_DIR="$PROJECT_DIR/logs"

mkdir -p "$LOG_DIR"

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║${BOLD}          🔑  BITCOIN PUZZLE HUNTER — LAUNCHER  🔑          ${NC}${CYAN}║${NC}"
echo -e "${CYAN}╠══════════════════════════════════════════════════════════════╣${NC}"
echo -e "${CYAN}║${NC}  Backend  : Rust (Axum) — http://0.0.0.0:3001              ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}  Frontend : Vite (React) — http://0.0.0.0:5173             ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}  Database : PostgreSQL   — localhost:5432                   ${CYAN}║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ────────────────────────────────────────────────────────────────
# 0. Load .env
# ────────────────────────────────────────────────────────────────
if [ -f ".env" ]; then
    echo -e "[ENV] Loading .env file..."
    export $(grep -v '^#' .env | grep -v '^$' | xargs)
    echo -e "[ENV] HUNT_MODE = ${YELLOW}${HUNT_MODE:-local}${NC}"
else
    echo -e "${YELLOW}[ENV] WARNING: .env file not found${NC}"
fi

# ────────────────────────────────────────────────────────────────
# 1. Stop previous instances
# ────────────────────────────────────────────────────────────────
echo ""
echo "[CLEANUP] Stopping previous instances..."

if [ -f "$BACKEND_PID_FILE" ]; then
    OLD_PID=$(cat "$BACKEND_PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        kill "$OLD_PID" 2>/dev/null || true
        sleep 1
        echo "[CLEANUP] Stopped old backend (PID: $OLD_PID)"
    fi
    rm -f "$BACKEND_PID_FILE"
fi

if [ -f "$FRONTEND_PID_FILE" ]; then
    OLD_PID=$(cat "$FRONTEND_PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        kill "$OLD_PID" 2>/dev/null || true
        echo "[CLEANUP] Stopped old frontend (PID: $OLD_PID)"
    fi
    rm -f "$FRONTEND_PID_FILE"
fi

# Also kill by port
lsof -ti:3001 2>/dev/null | xargs kill -9 2>/dev/null || true
lsof -ti:5173 2>/dev/null | xargs kill -9 2>/dev/null || true

echo -e "[CLEANUP] ${GREEN}✓ Clean${NC}"

# ────────────────────────────────────────────────────────────────
# 2. Check & Start PostgreSQL
# ────────────────────────────────────────────────────────────────
echo ""
echo "[DB] Checking PostgreSQL..."

if command -v docker &>/dev/null; then
    if docker ps --format '{{.Names}}' 2>/dev/null | grep -q "puzzle_postgres"; then
        echo -e "[DB] ${GREEN}✓ PostgreSQL container already running${NC}"
    else
        echo "[DB] Starting PostgreSQL container..."
        docker compose up -d postgres
        sleep 3
        echo -e "[DB] ${GREEN}✓ PostgreSQL started${NC}"
    fi
elif command -v pg_isready &>/dev/null; then
    if pg_isready -q; then
        echo -e "[DB] ${GREEN}✓ PostgreSQL is ready (system service)${NC}"
    else
        echo -e "${YELLOW}[DB] WARNING: PostgreSQL not ready. Start it manually.${NC}"
    fi
else
    echo -e "${YELLOW}[DB] WARNING: Cannot detect PostgreSQL. Make sure it is running.${NC}"
fi

# ────────────────────────────────────────────────────────────────
# 3. Check binary
# ────────────────────────────────────────────────────────────────
echo ""
if [ ! -f "target/release/puzzle-hunt" ]; then
    echo -e "${RED}[ERROR] Backend binary not found!${NC}"
    echo "[ERROR] Run: cargo build --release"
    exit 1
fi
echo -e "[BINARY] ${GREEN}✓ target/release/puzzle-hunt found${NC}"

# ────────────────────────────────────────────────────────────────
# 4. Check frontend
# ────────────────────────────────────────────────────────────────
if [ ! -d "frontend/node_modules" ]; then
    echo "[FRONTEND] Installing dependencies..."
    cd frontend && npm install && cd ..
fi
echo -e "[FRONTEND] ${GREEN}✓ Dependencies ready${NC}"

# ────────────────────────────────────────────────────────────────
# 5. Start Backend
# ────────────────────────────────────────────────────────────────
echo ""
echo "[BACKEND] Starting Rust backend server..."
nohup ./target/release/puzzle-hunt > "$LOG_DIR/backend.log" 2>&1 &
BACKEND_PID=$!
echo "$BACKEND_PID" > "$BACKEND_PID_FILE"
sleep 2

if kill -0 "$BACKEND_PID" 2>/dev/null; then
    echo -e "[BACKEND] ${GREEN}✓ Running on http://0.0.0.0:3001 (PID: $BACKEND_PID)${NC}"
else
    echo -e "${RED}[BACKEND] ✗ Failed to start! Check logs/backend.log${NC}"
    exit 1
fi

# ────────────────────────────────────────────────────────────────
# 6. Start Frontend
# ────────────────────────────────────────────────────────────────
echo "[FRONTEND] Starting Vite dev server..."
cd frontend
nohup npx vite --host 0.0.0.0 > "$LOG_DIR/frontend.log" 2>&1 &
FRONTEND_PID=$!
echo "$FRONTEND_PID" > "$FRONTEND_PID_FILE"
cd ..
sleep 3

if kill -0 "$FRONTEND_PID" 2>/dev/null; then
    echo -e "[FRONTEND] ${GREEN}✓ Running on http://0.0.0.0:5173 (PID: $FRONTEND_PID)${NC}"
else
    echo -e "${RED}[FRONTEND] ✗ Failed to start! Check logs/frontend.log${NC}"
fi

# ────────────────────────────────────────────────────────────────
# 7. Done
# ────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║${BOLD}                   ✅ ALL SERVICES STARTED                   ${NC}${GREEN}║${NC}"
echo -e "${GREEN}╠══════════════════════════════════════════════════════════════╣${NC}"
echo -e "${GREEN}║${NC}                                                            ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}  🌐 Dashboard : http://YOUR_IP:5173                        ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}  📡 API       : http://YOUR_IP:3001/api                    ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}  🔌 WebSocket : ws://YOUR_IP:3001/api/ws                   ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}  🗄️  Database  : localhost:5432                              ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}                                                            ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}  Backend PID  : $BACKEND_PID                                       ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}  Frontend PID : $FRONTEND_PID                                       ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}                                                            ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}  Logs: ./logs/backend.log & ./logs/frontend.log            ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}  Stop: ./stop.sh                                           ${GREEN}║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

