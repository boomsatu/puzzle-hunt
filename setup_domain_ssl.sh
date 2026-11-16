#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — DOMAIN & SSL SETUP SCRIPT
#   Domain : gpu-engine-one.bariscode.my.id
#   Target : http://localhost:3001
# ==============================================================================
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'

DOMAIN="gpu-engine-one.bariscode.my.id"
BACKEND_PORT=3001

echo -e "${CYAN}${BOLD}"
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║   🌐 PUZZLE HUNTER — SETUP DOMAIN & HTTPS SSL                       ║"
echo "║   Domain: $DOMAIN                       ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

echo -e "Pilih metode setup yang ingin Anda gunakan:"
echo -e "  ${GREEN}1) Cloudflare Tunnel (cloudflared) [SANGAT DIREKOMENDASIKAN UNTUK VAST.AI]${NC}"
echo -e "     - Bebas repot buka port di firewall Vast.ai (Bypass NAT)"
echo -e "     - Otomatis dapat SSL Valid (HTTPS & WSS) gratis dari Cloudflare"
echo -e "     - Vercel langsung bisa terhubung tanpa Mixed Content error"
echo -e "  ${YELLOW}2) Nginx Reverse Proxy + Let's Encrypt Certbot [UNTUK VPS STANDAR]${NC}"
echo -e "     - Membutuhkan port 80 & 443 terbuka langsung ke internet"
echo ""
read -p "Masukkan pilihan [1 atau 2] (Default: 1): " CHOICE
CHOICE=${CHOICE:-1}

if [ "$CHOICE" == "1" ]; then
    echo -e "\n${CYAN}━━━ Menyiapkan Cloudflare Tunnel (cloudflared) ━━━${NC}"
    
    # 1. Download & Install cloudflared
    if ! command -v cloudflared &>/dev/null; then
        echo -e "Mengunduh cloudflared binary..."
        ARCH=$(uname -m)
        if [ "$ARCH" = "x86_64" ]; then
            wget -q --show-progress https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.deb
            dpkg -i cloudflared-linux-amd64.deb || apt-get install -f -y
            rm -f cloudflared-linux-amd64.deb
        elif [ "$ARCH" = "aarch64" ]; then
            wget -q --show-progress https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-arm64.deb
            dpkg -i cloudflared-linux-arm64.deb || apt-get install -f -y
            rm -f cloudflared-linux-arm64.deb
        fi
    fi
    echo -e "${GREEN}✓ cloudflared terpasang: $(cloudflared --version)${NC}"

    echo -e "\n${YELLOW}Koneksi Cloudflare Tunnel dapat dilakukan dengan 2 cara:${NC}"
    echo -e "A. Masukkan Token Tunnel Cloudflare (Jika sudah membuat Tunnel di dash.cloudflare.com -> Zero Trust -> Tunnels)"
    echo -e "B. Menggunakan Quick Tunnel instan Cloudflare"
    echo ""
    read -p "Apakah Anda memiliki Cloudflare Tunnel Token? [y/N]: " HAS_TOKEN

    if [[ "$HAS_TOKEN" =~ ^[Yy]$ ]]; then
        read -p "Paste Cloudflare Tunnel Token Anda: " CF_TOKEN
        if [ -n "$CF_TOKEN" ]; then
            echo -e "Memasang cloudflared sebagai background service..."
            cloudflared service install "$CF_TOKEN" || true
            systemctl start cloudflared 2>/dev/null || nohup cloudflared tunnel run --token "$CF_TOKEN" > logs/cloudflared.log 2>&1 &
            echo -e "${GREEN}✅ Cloudflare Tunnel aktif! Domain https://$DOMAIN sudah terhubung ke backend.${NC}"
        fi
    else
        echo -e "\n${CYAN}Menjalankan Cloudflare Tunnel ke http://localhost:$BACKEND_PORT di background...${NC}"
        mkdir -p logs
        pkill -f cloudflared 2>/dev/null || true
        nohup cloudflared tunnel --url "http://localhost:$BACKEND_PORT" > logs/cloudflared.log 2>&1 &
        sleep 4
        TUNNEL_URL=$(grep -o 'https://[-a-zA-Z0-9.]*\.trycloudflare\.com' logs/cloudflared.log | head -1 || echo "")
        echo -e "${GREEN}✅ Tunnel berjalan di background!${NC}"
        if [ -n "$TUNNEL_URL" ]; then
            echo -e "🌐 Temporary HTTPS URL: ${BOLD}${GREEN}$TUNNEL_URL${NC}"
            echo -e "💡 Anda bisa membuat CNAME di Cloudflare DNS:"
            echo -e "   $DOMAIN  ->  ${TUNNEL_URL#https://}"
        fi
        echo -e "Cek log tunnel: tail -f logs/cloudflared.log"
    fi

elif [ "$CHOICE" == "2" ]; then
    echo -e "\n${CYAN}━━━ Menyiapkan Nginx Reverse Proxy + SSL ━━━${NC}"
    
    # 1. Install Nginx & Certbot
    apt-get update -qq
    apt-get install -y -qq nginx certbot python3-certbot-nginx

    # 2. Pasang konfigurasi Nginx
    PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    CONF_SRC="$PROJECT_DIR/nginx/gpu-engine.conf"
    CONF_DEST="/etc/nginx/sites-available/$DOMAIN"

    if [ -f "$CONF_SRC" ]; then
        cp "$CONF_SRC" "$CONF_DEST"
        ln -sf "$CONF_DEST" "/etc/nginx/sites-enabled/$DOMAIN"
        rm -f /etc/nginx/sites-enabled/default 2>/dev/null || true
    fi

    # 3. Test & Reload Nginx
    nginx -t
    systemctl reload nginx || service nginx reload

    echo -e "${GREEN}✓ Konfigurasi Nginx port 80 aktif untuk $DOMAIN.${NC}"
    echo -e "\n${CYAN}Memasang Sertifikat SSL Gratis (Let's Encrypt)...${NC}"
    read -p "Masukkan email Anda untuk sertifikat SSL (atau kosongkan untuk skip): " USER_EMAIL

    if [ -n "$USER_EMAIL" ]; then
        certbot --nginx -d "$DOMAIN" --non-interactive --agree-tos -m "$USER_EMAIL" || {
            echo -e "${YELLOW}⚠️ Certbot gagal mendapatkan sertifikat (mungkin port 80 belum tembus ke container).${NC}"
            echo -e "${YELLOW}Alternatif: Di Cloudflare Dashboard, aktifkan Proxy (Awan Oranye) dan set SSL Mode ke 'Flexible'. Cloudflare akan otomatis meng-handle HTTPS!${NC}"
        }
    else
        echo -e "${YELLOW}Certbot dilewati. Pastikan Proxy Cloudflare (Awan Oranye) aktif dengan mode SSL 'Flexible'.${NC}"
    fi

    echo -e "\n${GREEN}✅ Setup Nginx selesai! Domain https://$DOMAIN siap digunakan.${NC}"
fi

echo -e "\n${GREEN}══════════════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}Langkah Terakhir untuk Dashboard Vercel:${NC}"
echo -e "1. Buka project dashboard Anda di ${CYAN}Vercel${NC}"
echo -e "2. Masuk ke ${CYAN}Settings${NC} -> ${CYAN}Environment Variables${NC}"
echo -e "3. Ubah atau tambahkan:"
echo -e "   ${BOLD}VITE_API_URL = https://$DOMAIN${NC}"
echo -e "4. Redeploy Vercel. Dashboard Anda sekarang terhubung via HTTPS & WSS aman!"
echo -e "${GREEN}══════════════════════════════════════════════════════════════════════${NC}"
