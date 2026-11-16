#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — CUDA SOLVER COMPILER (KANGAROO CUDA)
#   Mengompilasi JeanLucPons Kangaroo untuk akselerasi GPU NVIDIA (Blackwell, Ada, Ampere)
# ==============================================================================
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║   ⚡ MEMASANG JEANLUCPONS KANGAROO CUDA ACCELERATOR SOLVER           ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"
mkdir -p "$PROJECT_DIR/bin"

# 1. Pastikan dependensi compiler & GMP terpasang
echo -e "${CYAN}[1/4] Memeriksa dependensi compiler & GMP...${NC}"
SUDO=""
if command -v sudo &>/dev/null; then SUDO="sudo"; fi

if command -v apt-get &>/dev/null; then
    $SUDO apt-get update -qq
    $SUDO apt-get install -y -qq build-essential libgmp-dev libssl-dev git wget
fi

# 2. Deteksi nvcc & CUDA SDK
echo -e "\n${CYAN}[2/4] Memeriksa CUDA Toolkit...${NC}"
if ! command -v nvcc &>/dev/null; then
    if [ -f "/usr/local/cuda/bin/nvcc" ]; then
        export PATH="/usr/local/cuda/bin:$PATH"
        export LD_LIBRARY_PATH="/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
    fi
fi

if command -v nvcc &>/dev/null; then
    NVCC_VER=$(nvcc --version | grep "release" | sed 's/.*release //' | sed 's/,.*//')
    echo -e "   NVCC Compiler : ${GREEN}$(which nvcc) (CUDA $NVCC_VER)${NC}"
else
    echo -e "${RED}❌ NVCC tidak ditemukan! Pastikan CUDA SDK terpasang.${NC}"
    exit 1
fi

CUDA_DIR="/usr/local/cuda"
if [ ! -d "$CUDA_DIR" ]; then
    CUDA_DIR=$(dirname $(dirname $(which nvcc)))
fi

# 3. Clone atau update JeanLucPons Kangaroo
echo -e "\n${CYAN}[3/4] Mengunduh repository JeanLucPons/Kangaroo...${NC}"
BUILD_DIR="/tmp/Kangaroo_build"
rm -rf "$BUILD_DIR"
git clone --depth 1 https://github.com/JeanLucPons/Kangaroo.git "$BUILD_DIR"
cd "$BUILD_DIR"

# 4. Compile dengan CUDA
echo -e "\n${CYAN}[4/4] Mengompilasi solver Kangaroo dengan GPU CUDA...${NC}"
make clean 2>/dev/null || true
make gpu=1 CUDA="$CUDA_DIR" all

# Salin binary ke lokasi yang dideteksi otomatis oleh engine
cp -f Kangaroo "$PROJECT_DIR/bin/kangaroo"
chmod +x "$PROJECT_DIR/bin/kangaroo"
$SUDO cp -f Kangaroo /usr/local/bin/kangaroo 2>/dev/null || true

echo -e "\n${GREEN}${BOLD}🎉 SELESAI! Binary Kangaroo CUDA siap di:${NC}"
echo -e "   📁 $PROJECT_DIR/bin/kangaroo"
echo -e "   📁 /usr/local/bin/kangaroo"
echo ""
"$PROJECT_DIR/bin/kangaroo" -h 2>&1 | head -5 || true
