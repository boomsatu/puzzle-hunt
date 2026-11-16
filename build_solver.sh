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
echo -e "${CYAN}[1/5] Memeriksa dependensi compiler & GMP...${NC}"
SUDO=""
if command -v sudo &>/dev/null; then SUDO="sudo"; fi

if command -v apt-get &>/dev/null; then
    $SUDO apt-get update -qq
    $SUDO apt-get install -y -qq build-essential libgmp-dev libssl-dev git wget python3
fi

# 2. Deteksi nvcc & CUDA SDK
echo -e "\n${CYAN}[2/5] Memeriksa CUDA Toolkit...${NC}"
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

HOST_CXX="$(which g++)"
echo -e "   Host C++ Comp : ${GREEN}$HOST_CXX${NC}"

# 3. Clone repository JeanLucPons/Kangaroo
echo -e "\n${CYAN}[3/5] Mengunduh repository JeanLucPons/Kangaroo...${NC}"
BUILD_DIR="/tmp/Kangaroo_build"
rm -rf "$BUILD_DIR"
git clone --depth 1 https://github.com/JeanLucPons/Kangaroo.git "$BUILD_DIR"
cd "$BUILD_DIR"

# 4. Patch compatibility untuk GCC 13/14 & CUDA 12/13
echo -e "\n${CYAN}[4/5] Menerapkan patch compatibility GCC 13/14 & CUDA 12/13...${NC}"
python3 -c "
import os, glob

# 4a. Tambahkan #include <cstdint> dan <stdint.h> ke seluruh headers & C++ source
patterns = ['**/*.h', '**/*.hpp', '**/*.cpp', '**/*.cu']
files = []
for p in patterns:
    files.extend(glob.glob(p, recursive=True))

patched = 0
for filepath in files:
    if os.path.isfile(filepath):
        with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
            content = f.read()
        if '#include <cstdint>' not in content and '#include <stdint.h>' not in content:
            new_content = '#include <cstdint>\n#include <stdint.h>\n' + content
            with open(filepath, 'w', encoding='utf-8') as f:
                f.write(new_content)
            patched += 1
print(f'   ✅ Berhasil mem-patch {patched} source files dengan <cstdint>')

# 4b. Perbaiki API usang/dihapus di CUDA 12/13 pada GPU/GPUEngine.cu
gpu_cu = 'GPU/GPUEngine.cu'
if os.path.isfile(gpu_cu):
    with open(gpu_cu, 'r', encoding='utf-8', errors='ignore') as f:
        cu_content = f.read()

    # computeMode dihapus di struct cudaDeviceProp pada CUDA 12 & 13
    cu_content = cu_content.replace('deviceProp.computeMode', '0')

    # cudaThreadSynchronize() diganti dengan cudaDeviceSynchronize()
    cu_content = cu_content.replace('cudaThreadSynchronize()', 'cudaDeviceSynchronize()')

    # Return default 128 cores/SM untuk arsitektur modern (Ampere, Ada, Hopper, Blackwell)
    cu_content = cu_content.replace('return 0;\n\n}', 'return 128;\n\n}')

    with open(gpu_cu, 'w', encoding='utf-8') as f:
        f.write(cu_content)
    print('   ✅ Berhasil mem-patch GPU/GPUEngine.cu untuk CUDA 12/13 (computeMode & core count)')
"

# 5. Deteksi Compute Capability GPU & Konfigurasi Makefile
echo -e "\n${CYAN}[5/5] Mengonfigurasi arsitektur GPU CUDA & Makefile...${NC}"
DETECTED_CCAP=$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader 2>/dev/null | tr -d '.' | head -n1 | tr -d '[:space:]')
echo -e "   Detected GPU CCAP: ${YELLOW}${DETECTED_CCAP:-unknown}${NC}"

TARGET_CCAP=""
if [ -n "$DETECTED_CCAP" ]; then
    if nvcc -arch=compute_${DETECTED_CCAP} -E -x cu /dev/null -o /dev/null 2>/dev/null; then
        TARGET_CCAP="$DETECTED_CCAP"
    fi
fi

if [ -z "$TARGET_CCAP" ]; then
    for arch in 120 100 90 89 86 80 75 70 61; do
        if nvcc -arch=compute_${arch} -E -x cu /dev/null -o /dev/null 2>/dev/null; then
            TARGET_CCAP="$arch"
            break
        fi
    done
fi

TARGET_CCAP="${TARGET_CCAP:-89}"
echo -e "   Target CUDA CCAP : ${GREEN}$TARGET_CCAP${NC}"

# Patch Makefile:
# 1) Perbarui CXXCUDA agar mengarah ke host g++ aktif
sed -i "s|CXXCUDA    = /usr/bin/g++-4.8|CXXCUDA    = $HOST_CXX|g" Makefile

# 2) Tambahkan include cstdint ke CXXFLAGS
sed -i 's/-DWITHGPU/-DWITHGPU -include cstdint -include stdint.h/g' Makefile

# 3) Tambahkan PTX fallback generator agar kompatibel dengan arsitektur GPU Blackwell/baru
sed -i "s/-gencode=arch=compute_\$(ccap),code=sm_\$(ccap)/-gencode=arch=compute_\$(ccap),code=sm_\$(ccap) -gencode=arch=compute_\$(ccap),code=compute_\$(ccap)/g" Makefile

# 6. Kompilasi
echo -e "\n${CYAN}🔨 Mengompilasi solver Kangaroo dengan GPU CUDA (ccap=$TARGET_CCAP)...${NC}"
make clean 2>/dev/null || true
make gpu=1 CUDA="$CUDA_DIR" CXXCUDA="$HOST_CXX" ccap="$TARGET_CCAP" all

# 7. Salin binary ke direktori puzzle-hunt dan sistem
echo -e "\n${CYAN}📦 Menginstal executable solver...${NC}"
SOLVER_BIN=""
if [ -f "kangaroo" ]; then
    SOLVER_BIN="kangaroo"
elif [ -f "Kangaroo" ]; then
    SOLVER_BIN="Kangaroo"
fi

if [ -z "$SOLVER_BIN" ]; then
    echo -e "${RED}❌ Kompilasi gagal: binary kangaroo tidak ditemukan!${NC}"
    exit 1
fi

cp -f "$SOLVER_BIN" "$PROJECT_DIR/bin/kangaroo"
chmod +x "$PROJECT_DIR/bin/kangaroo"
$SUDO cp -f "$SOLVER_BIN" /usr/local/bin/kangaroo 2>/dev/null || true
$SUDO chmod +x /usr/local/bin/kangaroo 2>/dev/null || true

echo -e "\n${GREEN}${BOLD}🎉 SELESAI! Binary Kangaroo CUDA berhasil dipasang:${NC}"
echo -e "   📁 $PROJECT_DIR/bin/kangaroo"
echo -e "   📁 /usr/local/bin/kangaroo"
echo ""
echo -e "${CYAN}Tes verifikasi help output:${NC}"
"$PROJECT_DIR/bin/kangaroo" -h 2>&1 | head -5 || true
