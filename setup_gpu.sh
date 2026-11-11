#!/usr/bin/env bash
set -e

echo "======================================================================"
echo "   🚀 BITCOIN PUZZLE HUNTER — GPU HOSTING AUTO SETUP (CUDA) 🚀"
echo "======================================================================"

# 1. Pastikan driver NVIDIA & CUDA terdeteksi
if ! command -v nvidia-smi &> /dev/null; then
    echo "❌ ERROR: 'nvidia-smi' tidak ditemukan. Pastikan GPU Hosting Anda memiliki driver NVIDIA."
    exit 1
fi

echo "🔍 Memeriksa GPU:"
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv

# 2. Pastikan dependensi kompilasi tersedia
echo "📦 Memeriksa dependensi sistem..."
if command -v apt-get &> /dev/null; then
    sudo apt-get update -qq && sudo apt-get install -y -qq git build-essential libgmp-dev
fi

# 3. Buat folder bin
mkdir -p bin

# 4. Clone & Compile Kangaroo CUDA (JeanLucPons)
if [ ! -f "bin/kangaroo" ]; then
    echo "⚙️ Mengunduh dan mengompilasi solver Kangaroo CUDA..."
    rm -rf /tmp/Kangaroo
    git clone https://github.com/JeanLucPons/Kangaroo.git /tmp/Kangaroo
    cd /tmp/Kangaroo
    make
    cp bin/Kangaroo "$OLDPWD/bin/kangaroo"
    cd "$OLDPWD"
    chmod +x bin/kangaroo
    echo "✅ Solver Kangaroo CUDA berhasil dipasang di bin/kangaroo!"
else
    echo "✅ Solver Kangaroo CUDA sudah terpasang di bin/kangaroo."
fi

# 5. Set .env ke mode GPU
if [ -f ".env" ]; then
    sed -i 's/^HUNT_MODE=.*/HUNT_MODE=gpu/' .env
    echo "✅ HUNT_MODE diatur ke 'gpu' di .env"
fi

# 6. Kompilasi Release Rust Backend
echo "🦀 Mengompilasi Rust Backend (Release Mode)..."
cargo build --release --bin puzzle-hunt

echo "======================================================================"
echo "🎉 SETUP GPU HOSTING SELESAI!"
echo "Jalankan hunter dengan: ./target/release/puzzle-hunt"
echo "======================================================================"

