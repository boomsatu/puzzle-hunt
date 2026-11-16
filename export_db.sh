#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — EXPORT DATABASE POSTGRESQL (LINUX / MAC)
#   Usage: bash export_db.sh
# ==============================================================================
set -e

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

echo -e "${CYAN}======================================================================${NC}"
echo -e "${CYAN}   📦 BITCOIN PUZZLE HUNTER — EXPORT DATABASE POSTGRESQL LOKAL        ${NC}"
echo -e "${CYAN}======================================================================${NC}"

mkdir -p data
BACKUP_FILE="data/puzzle_hunt_backup.sql"
BACKUP_GZ="data/puzzle_hunt_backup.sql.gz"

# 1. Cek apakah menggunakan Docker atau Native PostgreSQL
if command -v docker &>/dev/null && docker ps --format "{{.Names}}" 2>/dev/null | grep -q "puzzle_postgres"; then
    echo -e "${YELLOW}[*] Mengekspor dari Docker container 'puzzle_postgres'...${NC}"
    docker exec puzzle_postgres pg_dump -U postgres -d puzzle_hunt --clean --if-exists > "$BACKUP_FILE"
elif command -v pg_dump &>/dev/null; then
    echo -e "${YELLOW}[*] Mengekspor dari PostgreSQL Native di server...${NC}"
    PGPASSWORD=postgrespassword pg_dump -h localhost -U postgres -d puzzle_hunt --clean --if-exists > "$BACKUP_FILE" 2>/dev/null || \
    sudo -u postgres pg_dump -d puzzle_hunt --clean --if-exists > "$BACKUP_FILE"
else
    echo -e "${RED}❌ ERROR: Tidak dapat menemukan PostgreSQL (Docker maupun Native)!${NC}"
    exit 1
fi

if [ ! -s "$BACKUP_FILE" ]; then
    echo -e "${RED}❌ ERROR: File backup kosong! Pastikan database memiliki data.${NC}"
    exit 1
fi

# 2. Kompres ke format .gz agar transfer super cepat
gzip -kf "$BACKUP_FILE"

FILE_SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
GZ_SIZE=$(du -h "$BACKUP_GZ" | cut -f1)

echo -e "${GREEN}✅ Export berhasil!${NC}"
echo -e "   - SQL File  : $BACKUP_FILE ($FILE_SIZE)"
echo -e "   - GZIP File : ${BOLD}$BACKUP_GZ ($GZ_SIZE)${NC}"
echo ""
echo -e "${CYAN}======================================================================${NC}"
echo -e "${BOLD}3 CARA MUDAH MEMINDAHKAN KE SERVER BARU (RTX 5090):${NC}"
echo ""
echo -e "🔹 ${GREEN}OPSI 1: Upload Langsung (Paling Praktis Tanpa Download ke Laptop)${NC}"
echo -e "   Jalankan ini di server saat ini:"
echo -e "   ${YELLOW}curl --upload-file $BACKUP_GZ https://transfer.sh/puzzle_hunt_backup.sql.gz${NC}"
echo -e "   (Anda akan dapat 1 link download untuk langsung ditarik di server baru)"
echo ""
echo -e "🔹 ${GREEN}OPSI 2: Download ke Laptop via SCP (Terminal Laptop Anda):${NC}"
echo -e "   scp -P <PORT_SSH> root@<HOST_VAST>:/workspace/puzzle-hunt/$BACKUP_GZ ."
echo ""
echo -e "🔹 ${GREEN}OPSI 3: Di Server Baru Nanti, Cukup Jalankan:${NC}"
echo -e "   bash import_db.sh data/puzzle_hunt_backup.sql.gz"
echo -e "${CYAN}======================================================================${NC}"

