#!/usr/bin/env bash
# ==============================================================================
#   BITCOIN PUZZLE HUNTER — IMPORT DATABASE POSTGRESQL (CLOUD / GPU HOSTING)
#   Usage: bash import_db.sh [path/to/backup.sql]
# ==============================================================================
set -e

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

echo -e "${CYAN}======================================================================${NC}"
echo -e "${CYAN}   📥 BITCOIN PUZZLE HUNTER — IMPORT DATABASE KE CLOUD POSTGRESQL     ${NC}"
echo -e "${CYAN}======================================================================${NC}"

BACKUP_FILE="${1:-data/puzzle_hunt_backup.sql}"

if [ ! -f "$BACKUP_FILE" ]; then
    echo -e "${RED}❌ ERROR: File backup '$BACKUP_FILE' tidak ditemukan!${NC}"
    echo "   Pastikan file backup sudah di-copy dari lokal ke folder data/:"
    echo "   Contoh: data/puzzle_hunt_backup.sql"
    exit 1
fi

FILE_SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
echo -e "📦 File backup : ${YELLOW}$BACKUP_FILE ($FILE_SIZE)${NC}"

# 1. Pastikan Docker dan container PostgreSQL berjalan
if ! docker ps --format "{{.Names}}" | grep -q "puzzle_postgres"; then
    echo -e "${YELLOW}[*] Menyalakan container PostgreSQL...${NC}"
    docker compose up -d postgres
    echo "⏳ Menunggu PostgreSQL siap..."
    sleep 4
fi

# Tunggu pg_isready
MAX_WAIT=20
WAITED=0
until docker exec puzzle_postgres pg_isready -U postgres -d puzzle_hunt &>/dev/null || [ $WAITED -ge $MAX_WAIT ]; do
    echo "⏳ Menunggu database ready... ($WAITED/$MAX_WAIT)"
    sleep 2
    WAITED=$((WAITED+2))
done

if [ $WAITED -ge $MAX_WAIT ]; then
    echo -e "${RED}❌ Timeout: PostgreSQL belum siap di container puzzle_postgres.${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Container PostgreSQL aktif dan siap menerima data.${NC}"

# 2. Import database
echo -e "${YELLOW}[*] Mengimpor data ke PostgreSQL (mungkin butuh beberapa detik)...${NC}"
docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt < "$BACKUP_FILE" > /dev/null 2>&1 || {
    # Coba tanpa redirect dev/null jika ada issue
    docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt < "$BACKUP_FILE"
}

# 3. Verifikasi jumlah data
echo ""
echo -e "${CYAN}🔍 Memverifikasi hasil import:${NC}"
PUZZLE_COUNT=$(docker exec puzzle_postgres psql -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM puzzles;" | tr -d '[:space:]')
WINDOWS_COUNT=$(docker exec puzzle_postgres psql -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM scanned_windows;" | tr -d '[:space:]')
FOUND_COUNT=$(docker exec puzzle_postgres psql -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM found_keys;" | tr -d '[:space:]')

echo -e "   📋 Puzzles terdaftar      : ${GREEN}$PUZZLE_COUNT${NC}"
echo -e "   🔍 Scanned Windows riwayat : ${GREEN}$WINDOWS_COUNT${NC}"
echo -e "   🔑 Kunci ditemukan         : ${GREEN}$FOUND_COUNT${NC}"

echo ""
echo -e "${GREEN}🎉 IMPORT DATABASE BERHASIL! Seluruh progres lokal telah tersinkronisasi ke cloud.${NC}"
echo -e "${CYAN}======================================================================${NC}"

