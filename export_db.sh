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

if ! docker ps --format "{{.Names}}" | grep -q "puzzle_postgres"; then
    echo -e "${RED}❌ ERROR: Container 'puzzle_postgres' tidak berjalan!${NC}"
    echo "   Jalankan: docker compose up -d postgres"
    exit 1
fi

echo -e "${YELLOW}[*] Mengekspor database puzzle_hunt ke $BACKUP_FILE...${NC}"
docker exec puzzle_postgres pg_dump -U postgres -d puzzle_hunt --clean --if-exists > "$BACKUP_FILE"

FILE_SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
echo -e "${GREEN}✅ Export berhasil! Ukuran file: $FILE_SIZE ($BACKUP_FILE)${NC}"
echo ""
echo -e "${CYAN}Langkah selanjutnya untuk kirim ke GPU Hosting:${NC}"
echo "   scp $BACKUP_FILE user@<IP_GPU>:/path/to/puzzle-hunt/data/"
echo "   Lalu di server GPU jalankan: bash import_db.sh"
