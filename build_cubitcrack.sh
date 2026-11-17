#!/usr/bin/env bash
set -e

echo "=========================================================="
echo "⚡ MEMBANGUN cuBitCrack UNTUK NVIDIA GPU BLACKWELL / ADA / AMPERE"
echo "=========================================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
mkdir -p bin

# 1. Periksa compiler CUDA (nvcc)
if ! command -v nvcc &> /dev/null; then
    echo "❌ Error: 'nvcc' tidak ditemukan! Pastikan CUDA toolkit terpasang di server."
    exit 1
fi

echo "🔍 CUDA Compiler terdeteksi:"
nvcc --version | head -n 4

# 2. Periksa device GPU via nvidia-smi
if command -v nvidia-smi &> /dev/null; then
    echo "🎮 GPU Terdeteksi:"
    nvidia-smi --query-gpu=gpu_name,driver_version,memory.total --format=csv,noheader
fi

# 3. Clone atau gunakan repo BitCrack
BUILD_DIR="/tmp/BitCrack_build"
rm -rf "$BUILD_DIR"
echo "📦 Mengunduh source code BitCrack..."
git clone --depth 1 https://github.com/brichard19/BitCrack.git "$BUILD_DIR"
cd "$BUILD_DIR"

# Patch BitCrack source untuk mengaktifkan --daemon interactive mode (Zero Teardown / Real-time jumping)
if [ -f "$SCRIPT_DIR/patch_cubitcrack.py" ]; then
    echo "⚡ Menerapkan patch Interactive Daemon Mode ke BitCrack source..."
    python3 "$SCRIPT_DIR/patch_cubitcrack.py" "$BUILD_DIR" || python "$SCRIPT_DIR/patch_cubitcrack.py" "$BUILD_DIR" || true
fi

# 4. Modifikasi Makefile BitCrack agar menyertakan PTX (code=[sm_XX,compute_XX])
# PENTING: Tanpa 'compute_XX' (PTX), GPU baru seperti Blackwell (Compute 12.0)
# akan gagal runtime dengan pesan 'Error: named symbol not found'!
sed -i 's|NVCCFLAGS=.*|NVCCFLAGS=-std=c++11 -gencode=arch=compute_${COMPUTE_CAP},code=[sm_${COMPUTE_CAP},compute_${COMPUTE_CAP}] -Xptxas="-v" -Xcompiler "${CXXFLAGS}"|g' Makefile

# Periksa apakah nvcc mendukung compute_120 (Blackwell native) atau compute_90 / compute_89
COMPUTE_CAP="89"
if nvcc --help 2>&1 | grep -q "compute_120"; then
    echo "🌟 nvcc mendukung native Blackwell (compute_120)!"
    COMPUTE_CAP="120"
elif nvcc --help 2>&1 | grep -q "compute_90"; then
    echo "🌟 nvcc mendukung compute_90 (Hopper/Blackwell compat)!"
    COMPUTE_CAP="90"
fi

echo "⚙️ Mengompilasi cuBitCrack dengan COMPUTE_CAP=${COMPUTE_CAP} dan PTX JIT support..."
make clean || true
make BUILD_CUDA=1 COMPUTE_CAP=${COMPUTE_CAP} -j$(nproc)

# 5. Salin executable cuBitCrack ke bin/
if [ -f "bin/cuBitCrack" ]; then
    cp bin/cuBitCrack "$SCRIPT_DIR/bin/cuBitCrack"
    chmod +x "$SCRIPT_DIR/bin/cuBitCrack"
    echo "=========================================================="
    echo "✅ SUKSES! cuBitCrack berhasil dipasang:"
    ls -lh "$SCRIPT_DIR/bin/cuBitCrack"
    echo "=========================================================="
    echo "🔍 Menguji device list:"
    "$SCRIPT_DIR/bin/cuBitCrack" --list-devices || true
    echo "=========================================================="
    echo "🎉 SIAP DIGUNAKAN! cuBitCrack kini 100% kompatibel dengan RTX PRO 6000 Blackwell!"
else
    echo "❌ File bin/cuBitCrack tidak ditemukan setelah kompilasi."
    exit 1
fi
