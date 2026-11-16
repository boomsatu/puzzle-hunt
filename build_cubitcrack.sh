#!/usr/bin/env bash
set -e

echo "=========================================================="
echo "âš¡ MEMBANGUN cuBitCrack UNTUK NVIDIA GPU BLACKWELL / ADA / AMPERE"
echo "=========================================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
mkdir -p bin

# 1. Periksa compiler CUDA (nvcc)
if ! command -v nvcc &> /dev/null; then
    echo "âŒ Error: 'nvcc' tidak ditemukan! Pastikan CUDA toolkit terpasang di server."
    exit 1
fi

echo "ðŸ” CUDA Compiler terdeteksi:"
nvcc --version | head -n 4

# 2. Periksa device GPU via nvidia-smi
if command -v nvidia-smi &> /dev/null; then
    echo "ðŸŽ® GPU Terdeteksi:"
    nvidia-smi --query-gpu=gpu_name,driver_version,memory.total --format=csv,noheader
fi

# 3. Clone atau gunakan repo BitCrack
BUILD_DIR="/tmp/BitCrack_build"
rm -rf "$BUILD_DIR"
echo "ðŸ“¦ Mengunduh source code BitCrack..."
git clone --depth 1 https://github.com/brichard19/BitCrack.git "$BUILD_DIR"
cd "$BUILD_DIR"

# 4. Modifikasi Makefile BitCrack agar menyertakan PTX (code=[sm_XX,compute_XX])
# PENTING: Tanpa 'compute_XX' (PTX), GPU baru seperti Blackwell (Compute 12.0)
# akan gagal runtime dengan pesan 'Error: named symbol not found'!
sed -i 's|NVCCFLAGS=.*|NVCCFLAGS=-std=c++11 -gencode=arch=compute_${COMPUTE_CAP},code=[sm_${COMPUTE_CAP},compute_${COMPUTE_CAP}] -Xptxas="-v" -Xcompiler "${CXXFLAGS}"|g' Makefile

# Periksa apakah nvcc mendukung compute_120 (Blackwell native) atau compute_90 / compute_89
COMPUTE_CAP="89"
if nvcc --help 2>&1 | grep -q "compute_120"; then
    echo "ðŸŒŸ nvcc mendukung native Blackwell (compute_120)!"
    COMPUTE_CAP="120"
elif nvcc --help 2>&1 | grep -q "compute_90"; then
    echo "ðŸŒŸ nvcc mendukung compute_90 (Hopper/Blackwell compat)!"
    COMPUTE_CAP="90"
fi

echo "âš™ï¸ Mengompilasi cuBitCrack dengan COMPUTE_CAP=${COMPUTE_CAP}..."
make clean || true
make BUILD_CUDA=1 COMPUTE_CAP=${COMPUTE_CAP} -j$(nproc)

# 5. Salin executable cuBitCrack ke bin/
if [ -f "bin/cuBitCrack" ]; then
    cp bin/cuBitCrack "$SCRIPT_DIR/bin/cuBitCrack"
    chmod +x "$SCRIPT_DIR/bin/cuBitCrack"
    echo "=========================================================="
    echo "âœ… SUKSES! cuBitCrack berhasil dipasang:"
    ls -lh "$SCRIPT_DIR/bin/cuBitCrack"
    echo "=========================================================="
    echo "ðŸ” Menguji device list:"
    "$SCRIPT_DIR/bin/cuBitCrack" --list-devices || true
    echo "=========================================================="
    echo "ðŸŽ‰ SIAP DIGUNAKAN! cuBitCrack kini 100% kompatibel dengan RTX PRO 6000 Blackwell!"
else
    echo "âŒ File bin/cuBitCrack tidak ditemukan setelah kompilasi."
    exit 1
fi
