#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — KANGAROO CUDA RUNNER
#   Optimized for: NVIDIA RTX 6000 S / Blackwell / 96GB GDDR7
#   Usage: bash run_kangaroo.sh [puzzle_number]
#
#   Contoh:  bash run_kangaroo.sh 71
# ==============================================================================
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KANGAROO="$PROJECT_DIR/bin/kangaroo"

# ── Puzzle Ranges & Public Keys (Bitcoin Puzzle) ─────────────────────────────
# Format: PUZZLE_<num>_RANGE_START, PUZZLE_<num>_RANGE_END, PUZZLE_<num>_PUBKEY
declare -A PUZZLE_RANGES
declare -A PUZZLE_PUBKEYS

# Puzzle #71 (71-bit keyspace)
PUZZLE_RANGES[71_start]="380000000000000000"
PUZZLE_RANGES[71_end]="3fffffffffffffff"
PUZZLE_PUBKEYS[71]="03a2efa402fd5268400c77c20e574ba86409ededee7c4020e4b9f0edbee53de0d4"

# Puzzle #72 (72-bit keyspace)
PUZZLE_RANGES[72_start]="800000000000000000"
PUZZLE_RANGES[72_end]="ffffffffffffffffffff"
PUZZLE_PUBKEYS[72]="02e0a8b039282faf6fe0fd769cfbc4b6b4cf8758ba68220eac420e32b91ddfa673"

# Puzzle #73
PUZZLE_RANGES[73_start]="1000000000000000000"
PUZZLE_RANGES[73_end]="1ffffffffffffffffffff"
PUZZLE_PUBKEYS[73]="03c587f7a1e0e3e3bb49e5a52a1fae31a4ffe65dc76fc5c2f6e5879e37a51d96c7"

# ── Default: Puzzle #71 ───────────────────────────────────────────────────────
PUZZLE=${1:-71}
RANGE_START="${PUZZLE_RANGES[${PUZZLE}_start]}"
RANGE_END="${PUZZLE_RANGES[${PUZZLE}_end]}"
PUBKEY="${PUZZLE_PUBKEYS[$PUZZLE]}"

if [ -z "$RANGE_START" ] || [ -z "$PUBKEY" ]; then
  echo -e "${RED}❌ Puzzle #$PUZZLE tidak ada di database script ini.${NC}"
  echo "   Tersedia: 71, 72, 73"
  exit 1
fi

# ── GPU Detection & Config ────────────────────────────────────────────────────
GPU_NAME=$(nvidia-smi --query-gpu=name --format=csv,noheader | head -1)
GPU_MEM=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader | head -1)
GPU_UTIL=$(nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader | head -1)

echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║   🦘  KANGAROO CUDA SOLVER — Puzzle #$PUZZLE                           ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo -e "GPU    : ${GREEN}$GPU_NAME ($GPU_MEM)${NC}"
echo -e "Target : ${YELLOW}Puzzle #$PUZZLE${NC}"
echo -e "Range  : $RANGE_START → $RANGE_END"
echo -e "PubKey : $PUBKEY"
echo ""

# Optimal settings untuk RTX 6000 S (Blackwell 96GB)
if echo "$GPU_NAME" | grep -qi "6000"; then
  GRID_X=188        # 188 SMs
  GRID_Y=256        # 256 threads/SM
  DP_BITS=24        # Distinguished point bitlen (96GB VRAM bisa handle besar)
  echo -e "${GREEN}⚡ RTX 6000 S profile: Grid=${GRID_X}×${GRID_Y}, DP=$DP_BITS${NC}"
elif echo "$GPU_NAME" | grep -qi "4090"; then
  GRID_X=128; GRID_Y=256; DP_BITS=22
  echo -e "${GREEN}⚡ RTX 4090 profile${NC}"
elif echo "$GPU_NAME" | grep -qi "3090"; then
  GRID_X=82; GRID_Y=256; DP_BITS=21
  echo -e "${GREEN}⚡ RTX 3090 profile${NC}"
else
  GRID_X=64; GRID_Y=128; DP_BITS=20
  echo -e "${YELLOW}⚡ Generic GPU profile${NC}"
fi

echo ""
echo -e "${YELLOW}📊 Estimasi Speed: ~3.5–5.5 Gkeys/s${NC}"
echo -e "${YELLOW}📊 Estimasi Waktu: ~2–4 jam untuk Puzzle #$PUZZLE${NC}"
echo ""
echo -e "${CYAN}🚀 Memulai Kangaroo CUDA...${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Jalankan solver
"$KANGAROO" \
  -t 0 \
  -gpu \
  -gpuId 0 \
  -d "$DP_BITS" \
  -grid "$GRID_X" "$GRID_Y" \
  "$RANGE_START" "$RANGE_END" \
  "$PUBKEY"

echo ""
echo -e "${GREEN}✅ Solver selesai!${NC}"
