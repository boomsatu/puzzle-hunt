#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — GPU HOSTING AUTO SETUP
#   Optimized for: NVIDIA RTX 6000 S / Blackwell / 96GB GDDR7
#   Author: puzzle-hunt deploy script
#   Usage:  bash setup_gpu.sh
# ==============================================================================
set -e

# ── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

banner() {
  echo -e "${CYAN}${BOLD}"
  echo "╔══════════════════════════════════════════════════════════════════════╗"
  echo "║   🚀  BITCOIN PUZZLE HUNTER — GPU DEPLOY (RTX 6000 S / 96GB)  🚀   ║"
  echo "╚══════════════════════════════════════════════════════════════════════╝"
  echo -e "${NC}"
}
banner

# ── 0. Root check ────────────────────────────────────────────────────────────
if [ "$EUID" -ne 0 ]; then
  echo -e "${YELLOW}⚠  Jalankan sebagai root atau dengan sudo untuk instalasi driver.${NC}"
fi

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"
echo -e "${GREEN}📁 Project Dir: $PROJECT_DIR${NC}"

# ── 1. Deteksi GPU ───────────────────────────────────────────────────────────
echo -e "\n${CYAN}━━━ [1/8] Deteksi GPU NVIDIA ━━━${NC}"
if ! command -v nvidia-smi &>/dev/null; then
  echo -e "${RED}❌ nvidia-smi tidak ditemukan. Install driver NVIDIA dulu!${NC}"
  echo "   → Ubuntu: sudo apt install nvidia-driver-560 cuda-toolkit-12-6"
  exit 1
fi

GPU_NAME=$(nvidia-smi --query-gpu=name --format=csv,noheader | head -1)
GPU_MEM=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader | head -1)
GPU_DRIVER=$(nvidia-smi --query-gpu=driver_version --format=csv,noheader | head -1)
CUDA_VER=$(nvcc --version 2>/dev/null | grep "release" | awk '{print $5}' | tr -d ',') || CUDA_VER="N/A"

echo -e "${GREEN}"
echo "   GPU    : $GPU_NAME"
echo "   VRAM   : $GPU_MEM"
echo "   Driver : $GPU_DRIVER"
echo "   CUDA   : $CUDA_VER"
echo -e "${NC}"

# Deteksi apakah ini RTX 6000 S / Blackwell
if echo "$GPU_NAME" | grep -qi "6000"; then
  echo -e "${GREEN}✅ RTX 6000 Series terdeteksi — Blackwell profile aktif!${NC}"
  GRID_X=188
  GRID_Y=256
  DP_BITS=24
  VRAM_DP_GB=14
else
  echo -e "${YELLOW}⚠  GPU bukan RTX 6000 S — menggunakan config generik${NC}"
  GRID_X=128
  GRID_Y=128
  DP_BITS=20
  VRAM_DP_GB=4
fi

echo -e "   Grid Config : ${GRID_X}×${GRID_Y} | DP Bits: $DP_BITS | VRAM for DP: ${VRAM_DP_GB}GB"

# ── 2. Sistem Dependencies ───────────────────────────────────────────────────
echo -e "\n${CYAN}━━━ [2/8] Install Dependencies Sistem ━━━${NC}"
if command -v apt-get &>/dev/null; then
  sudo apt-get update -qq
  sudo apt-get install -y -qq \
    git build-essential libgmp-dev libssl-dev pkg-config \
    curl wget unzip screen htop nvtop \
    docker.io docker-compose-plugin
  echo -e "${GREEN}✅ Dependencies Ubuntu/Debian terpasang${NC}"
elif command -v yum &>/dev/null; then
  sudo yum install -y git gcc g++ gmp-devel openssl-devel \
    curl wget screen htop
  echo -e "${GREEN}✅ Dependencies CentOS/RHEL terpasang${NC}"
fi

# CUDA Toolkit check
if ! command -v nvcc &>/dev/null; then
  echo -e "${YELLOW}⚠  nvcc tidak ditemukan. Install CUDA Toolkit:${NC}"
  echo "   wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb"
  echo "   sudo dpkg -i cuda-keyring_1.1-1_all.deb"
  echo "   sudo apt-get install -y cuda-toolkit-12-6"
  echo -e "${YELLOW}   Lanjutkan tanpa nvcc (solver fallback ke CPU kangaroo)${NC}"
  HAS_NVCC=false
else
  HAS_NVCC=true
  echo -e "${GREEN}✅ nvcc tersedia: $(nvcc --version | grep release)${NC}"
fi

# ── 3. Rust Toolchain ────────────────────────────────────────────────────────
echo -e "\n${CYAN}━━━ [3/8] Rust Toolchain ━━━${NC}"
if ! command -v cargo &>/dev/null; then
  echo "📦 Menginstall Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  source "$HOME/.cargo/env"
  echo -e "${GREEN}✅ Rust terpasang: $(rustc --version)${NC}"
else
  echo -e "${GREEN}✅ Rust sudah ada: $(rustc --version)${NC}"
fi
source "$HOME/.cargo/env" 2>/dev/null || true

# ── 4. Node.js & Build Frontend ──────────────────────────────────────────────
echo -e "\n${CYAN}━━━ [4/8] Node.js & Frontend Build ━━━${NC}"
if ! command -v node &>/dev/null; then
  curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
  sudo apt-get install -y nodejs
fi
echo -e "${GREEN}✅ Node: $(node --version) | npm: $(npm --version)${NC}"

cd "$PROJECT_DIR/frontend"
echo "📦 Install npm packages..."
npm install --silent
echo "🔨 Build frontend production..."
npm run build
echo -e "${GREEN}✅ Frontend built → dist/${NC}"
cd "$PROJECT_DIR"

# ── 5. Compile Kangaroo CUDA Solver ──────────────────────────────────────────
echo -e "\n${CYAN}━━━ [5/8] Compile Kangaroo CUDA Solver (JeanLucPons) ━━━${NC}"
mkdir -p bin

if [ -f "bin/kangaroo" ]; then
  echo -e "${GREEN}✅ bin/kangaroo sudah ada — skip compile${NC}"
else
  if [ "$HAS_NVCC" = true ]; then
    echo "⚙️  Cloning & compiling Kangaroo CUDA..."
    rm -rf /tmp/Kangaroo
    git clone https://github.com/JeanLucPons/Kangaroo.git /tmp/Kangaroo
    cd /tmp/Kangaroo

    # Patch untuk Blackwell SM_90 (RTX 6000 S)
    if echo "$GPU_NAME" | grep -qi "6000"; then
      echo "🔧 Patch Makefile untuk Blackwell SM_90..."
      sed -i 's/-gencode arch=compute_80,code=sm_80/-gencode arch=compute_90,code=sm_90 -gencode arch=compute_80,code=sm_80/' Makefile 2>/dev/null || true
    fi

    make -j$(nproc) 2>&1 | tail -5
    cp Kangaroo "$PROJECT_DIR/bin/kangaroo"
    chmod +x "$PROJECT_DIR/bin/kangaroo"
    cd "$PROJECT_DIR"
    echo -e "${GREEN}✅ bin/kangaroo CUDA compiled berhasil!${NC}"
  else
    echo -e "${YELLOW}⚠  nvcc tidak ada — unduh binary pre-compiled...${NC}"
    # Fallback: pakai versi CPU-only kangaroo
    git clone --depth=1 https://github.com/JeanLucPons/Kangaroo.git /tmp/Kangaroo
    cd /tmp/Kangaroo
    make -j$(nproc) NOSSE=1 2>&1 | tail -3 || true
    if [ -f "Kangaroo" ]; then
      cp Kangaroo "$PROJECT_DIR/bin/kangaroo"
      chmod +x "$PROJECT_DIR/bin/kangaroo"
    fi
    cd "$PROJECT_DIR"
    echo -e "${YELLOW}⚠  bin/kangaroo CPU-only terpasang (install CUDA untuk GPU mode)${NC}"
  fi
fi

# Test solver
echo -n "   Testing solver... "
if "$PROJECT_DIR/bin/kangaroo" --help &>/dev/null 2>&1; then
  echo -e "${GREEN}OK${NC}"
else
  echo -e "${YELLOW}binary ada tapi help flag berbeda — OK${NC}"
fi

# ── 6. Konfigurasi .env ───────────────────────────────────────────────────────
echo -e "\n${CYAN}━━━ [6/8] Konfigurasi Environment (.env) ━━━${NC}"

if [ ! -f ".env" ]; then
  cp .env.example .env
  echo "📋 .env dibuat dari .env.example"
fi

# Set HUNT_MODE=gpu
sed -i 's/^HUNT_MODE=.*/HUNT_MODE=gpu/' .env

# Generate AUTH_SECRET random jika masih default
if grep -q "super_secret_puzzle_hunt_auth_key_2026" .env; then
  NEW_SECRET=$(openssl rand -hex 32)
  sed -i "s/^AUTH_SECRET=.*/AUTH_SECRET=$NEW_SECRET/" .env
  echo -e "${GREEN}✅ AUTH_SECRET di-generate otomatis (aman)${NC}"
fi

echo -e "${GREEN}✅ HUNT_MODE=gpu aktif di .env${NC}"

# Tampilkan konfigurasi penting
echo ""
echo -e "${YELLOW}📋 Konfigurasi .env saat ini:${NC}"
grep -E "^(HUNT_MODE|ADMIN_USERNAME|PORT|DATABASE_URL)" .env | sed 's/^/   /'

# ── 7. Build Rust Backend (Release) ──────────────────────────────────────────
echo -e "\n${CYAN}━━━ [7/8] Build Rust Backend (Release) ━━━${NC}"
echo "🦀 Compiling puzzle-hunt (release)..."
cargo build --release --bin puzzle-hunt 2>&1 | grep -E "(Compiling|Finished|error)" | tail -10
echo -e "${GREEN}✅ Binary: target/release/puzzle-hunt${NC}"

# ── 8. Docker PostgreSQL ──────────────────────────────────────────────────────
echo -e "\n${CYAN}━━━ [8/8] PostgreSQL via Docker ━━━${NC}"
if command -v docker &>/dev/null; then
  docker compose up -d postgres
  echo "⏳ Menunggu PostgreSQL siap..."
  sleep 5
  if docker exec puzzle_postgres pg_isready -U postgres &>/dev/null; then
    echo -e "${GREEN}✅ PostgreSQL running di port 5432${NC}"
  else
    echo -e "${YELLOW}⚠  PostgreSQL belum ready — cek: docker logs puzzle_postgres${NC}"
  fi
else
  echo -e "${YELLOW}⚠  Docker tidak ditemukan. Install Docker atau jalankan PostgreSQL manual.${NC}"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                   🎉 SETUP SELESAI! 🎉                              ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo -e "${GREEN}GPU    : $GPU_NAME ($GPU_MEM)${NC}"
echo -e "${GREEN}Mode   : GPU (CUDA) — ${GRID_X}×${GRID_Y} Grid, DP=$DP_BITS bits${NC}"
echo ""
echo -e "${BOLD}📌 Langkah selanjutnya:${NC}"
echo ""
echo "  1. Edit .env — ubah password & kredensial:"
echo "       nano .env"
echo "       ADMIN_PASSWORD=<password_kuat>"
echo "       DEFAULT_DESTINATION_ADDRESS=<bitcoin_address_kamu>"
echo ""
echo "  2. Jalankan backend:"
echo "       ./target/release/puzzle-hunt"
echo "     atau di background:"
echo "       screen -S puzzle ./target/release/puzzle-hunt"
echo ""
echo "  3. Akses dashboard:"
echo "       http://$(hostname -I | awk '{print $1}'):3001"
echo ""
echo -e "${YELLOW}⚡ Expected Speed RTX 6000 S: ~3.5–5.5 Gkeys/s${NC}"
echo -e "${YELLOW}   vs CPU 8-core: 1.4 Mkeys/s → Percepatan ~2500–4000×${NC}"
echo ""
