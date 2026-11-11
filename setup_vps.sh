#!/usr/bin/env bash
set -e

echo "======================================================================"
echo "   ⚡ BITCOIN PUZZLE HUNTER — VPS HOSTING AUTO SETUP (CPU MAX) ⚡"
echo "======================================================================"

# 1. Update paket sistem
if command -v apt-get &> /dev/null; then
    sudo apt-get update -qq && sudo apt-get install -y -qq git build-essential pkg-config libssl-dev postgresql-client
fi

# 2. Set .env ke mode VPS
if [ -f ".env" ]; then
    sed -i 's/^HUNT_MODE=.*/HUNT_MODE=vps/' .env
    echo "✅ HUNT_MODE diatur ke 'vps' di .env (Max CPU Cores - 1 untuk OS)"
fi

# 3. Kompilasi Release Rust Backend
echo "🦀 Mengompilasi Rust Backend (Release Mode)..."
cargo build --release --bin puzzle-hunt

echo "======================================================================"
echo "🎉 SETUP VPS HOSTING SELESAI!"
echo "Jalankan hunter dengan: ./target/release/puzzle-hunt"
echo "======================================================================"

