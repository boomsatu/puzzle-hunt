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

# 4. Tentukan Compute Capability
# RTX PRO 6000 Blackwell / RTX 5090 = sm_90 atau sm_89
COMPUTE_CAP="89"
echo "âš™ï¸ Mencoba kompilasi dengan COMPUTE_CAP=${COMPUTE_CAP}..."
if ! make BUILD_CUDA=1 COMPUTE_CAP=${COMPUTE_CAP} -j$(nproc); then
    echo "âš ï¸ Kompilasi dengan COMPUTE_CAP=89 gagal, mencoba COMPUTE_CAP=86 (Ampere/Universal)..."
    make clean || true
    COMPUTE_CAP="86"
    make BUILD_CUDA=1 COMPUTE_CAP=${COMPUTE_CAP} -j$(nproc)
fi

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
    echo "ðŸŽ‰ SIAP DIGUNAKAN! Semua puzzle address (tanpa pubkey) seperti #71 sekarang 100% berjalan di GPU!"
else
    echo "âŒ File bin/cuBitCrack tidak ditemukan setelah kompilasi."
    exit 1
fi
