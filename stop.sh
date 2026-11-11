#!/bin/bash
# ══════════════════════════════════════════════════════════════════
# 🛑 Bitcoin Puzzle Hunter — Stop All Services (Linux)
# ══════════════════════════════════════════════════════════════════

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'
BOLD='\033[1m'

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_DIR"

BACKEND_PID_FILE="$PROJECT_DIR/.backend.pid"
FRONTEND_PID_FILE="$PROJECT_DIR/.frontend.pid"

echo ""
echo -e "${RED}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${RED}║${BOLD}         🛑  BITCOIN PUZZLE HUNTER — STOP ALL  🛑           ${NC}${RED}║${NC}"
echo -e "${RED}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ────────────────────────────────────────────────────────────────
# 1. Stop Backend
# ────────────────────────────────────────────────────────────────
echo "[BACKEND] Stopping puzzle-hunt..."
if [ -f "$BACKEND_PID_FILE" ]; then
    PID=$(cat "$BACKEND_PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        kill "$PID" 2>/dev/null
        sleep 1
        # Force kill if still running
        kill -0 "$PID" 2>/dev/null && kill -9 "$PID" 2>/dev/null
        echo -e "[BACKEND] ${GREEN}✓ Stopped (PID: $PID)${NC}"
    else
        echo "[BACKEND] — Not running (stale PID file)"
    fi
    rm -f "$BACKEND_PID_FILE"
else
    # Try by port
    PIDS=$(lsof -ti:3001 2>/dev/null)
    if [ -n "$PIDS" ]; then
        echo "$PIDS" | xargs kill -9 2>/dev/null
        echo -e "[BACKEND] ${GREEN}✓ Stopped (port 3001)${NC}"
    else
        echo "[BACKEND] — Not running"
    fi
fi

# ────────────────────────────────────────────────────────────────
# 2. Stop Frontend
# ────────────────────────────────────────────────────────────────
echo ""
echo "[FRONTEND] Stopping Vite dev server..."
if [ -f "$FRONTEND_PID_FILE" ]; then
    PID=$(cat "$FRONTEND_PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        kill "$PID" 2>/dev/null
        sleep 1
        kill -0 "$PID" 2>/dev/null && kill -9 "$PID" 2>/dev/null
        echo -e "[FRONTEND] ${GREEN}✓ Stopped (PID: $PID)${NC}"
    else
        echo "[FRONTEND] — Not running (stale PID file)"
    fi
    rm -f "$FRONTEND_PID_FILE"
else
    PIDS=$(lsof -ti:5173 2>/dev/null)
    if [ -n "$PIDS" ]; then
        echo "$PIDS" | xargs kill -9 2>/dev/null
        echo -e "[FRONTEND] ${GREEN}✓ Stopped (port 5173)${NC}"
    else
        echo "[FRONTEND] — Not running"
    fi
fi

# ────────────────────────────────────────────────────────────────
# 3. Stop PostgreSQL (optional)
# ────────────────────────────────────────────────────────────────
echo ""
read -p "[DB] Stop PostgreSQL container juga? (y/N): " STOP_DB
if [ "$STOP_DB" = "y" ] || [ "$STOP_DB" = "Y" ]; then
    docker compose down 2>/dev/null
    echo -e "[DB] ${GREEN}✓ PostgreSQL stopped${NC}"
else
    echo "[DB] — PostgreSQL tetap berjalan"
fi

# ────────────────────────────────────────────────────────────────
# Done
# ────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║${BOLD}              ✅ ALL SERVICES STOPPED                        ${NC}${GREEN}║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

