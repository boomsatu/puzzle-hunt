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

# Jika input berupa URL (http / https), unduh otomatis ke folder data/
if [[ "$BACKUP_FILE" =~ ^https?:// ]]; then
    echo -e "${YELLOW}🌐 Mendeteksi tautan download Filebase/Cloud: ${BACKUP_FILE}${NC}"
    mkdir -p data
    DOWNLOAD_PATH="data/puzzle_hunt_backup.sql.gz"
    echo -e "⏳ Mengunduh file backup dari cloud..."
    curl -L --progress-bar -o "$DOWNLOAD_PATH" "$BACKUP_FILE"
    BACKUP_FILE="$DOWNLOAD_PATH"
    echo -e "${GREEN}✅ File berhasil diunduh ke: $BACKUP_FILE${NC}"
fi

# Cek apakah ada file .sql atau .sql.gz
if [ ! -f "$BACKUP_FILE" ]; then
    if [ -f "data/puzzle_hunt_backup.sql.gz" ]; then
        BACKUP_FILE="data/puzzle_hunt_backup.sql.gz"
    elif [ -f "data/puzzle_hunt_backup.sql" ]; then
        BACKUP_FILE="data/puzzle_hunt_backup.sql"
    else
        echo -e "${RED}❌ ERROR: File backup '$BACKUP_FILE' tidak ditemukan!${NC}"
        echo "   Pastikan file backup sudah ada di folder data/:"
        echo "   Contoh: data/puzzle_hunt_backup.sql atau data/puzzle_hunt_backup.sql.gz"
        exit 1
    fi
fi

FILE_SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
echo -e "📦 File backup : ${YELLOW}$BACKUP_FILE ($FILE_SIZE)${NC}"

# 1. Pastikan PostgreSQL berjalan (Docker atau Native)
IS_DOCKER=false
if command -v docker &>/dev/null && docker ps &>/dev/null; then
    IS_DOCKER=true
    if ! docker ps --format "{{.Names}}" | grep -q "puzzle_postgres"; then
        echo -e "${YELLOW}[*] Menyalakan container PostgreSQL...${NC}"
        docker compose up -d postgres
        echo "⏳ Menunggu PostgreSQL siap..."
        sleep 4
    fi

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
else
    # Native PostgreSQL
    service postgresql start 2>/dev/null || sudo service postgresql start 2>/dev/null || true
    echo -e "${GREEN}✅ PostgreSQL (Native) siap menerima data.${NC}"
fi

# 2. Import database (mendukung .sql biasa maupun .sql.gz)
echo -e "${YELLOW}[*] Mengimpor data ke PostgreSQL (mungkin butuh beberapa detik)...${NC}"

IS_GZIP=false
if [[ "$BACKUP_FILE" == *.gz ]] || (command -v file &>/dev/null && file "$BACKUP_FILE" | grep -qi "gzip") || (gzip -t "$BACKUP_FILE" 2>/dev/null); then
    IS_GZIP=true
fi

if [ "$IS_DOCKER" = true ]; then
    if [ "$IS_GZIP" = true ]; then
        gunzip -c "$BACKUP_FILE" | docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt > /dev/null 2>&1 || \
        gunzip -c "$BACKUP_FILE" | docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt
    else
        docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt < "$BACKUP_FILE" > /dev/null 2>&1 || \
        docker exec -i puzzle_postgres psql -U postgres -d puzzle_hunt < "$BACKUP_FILE"
    fi
else
    export PGPASSWORD=postgrespassword
    if [ "$IS_GZIP" = true ]; then
        gunzip -c "$BACKUP_FILE" | psql -h localhost -U postgres -d puzzle_hunt > /dev/null 2>&1 || \
        gunzip -c "$BACKUP_FILE" | sudo -u postgres psql -d puzzle_hunt 2>/dev/null || \
        gunzip -c "$BACKUP_FILE" | su - postgres -c "psql -d puzzle_hunt"
    else
        psql -h localhost -U postgres -d puzzle_hunt < "$BACKUP_FILE" > /dev/null 2>&1 || \
        sudo -u postgres psql -d puzzle_hunt < "$BACKUP_FILE" 2>/dev/null || \
        su - postgres -c "psql -d puzzle_hunt" < "$BACKUP_FILE"
    fi
fi

# 3. Verifikasi jumlah data
echo ""
echo -e "${CYAN}🔍 Memverifikasi hasil import:${NC}"
if [ "$IS_DOCKER" = true ]; then
    PUZZLE_COUNT=$(docker exec puzzle_postgres psql -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM puzzles;" 2>/dev/null | tr -d '[:space:]')
    WINDOWS_COUNT=$(docker exec puzzle_postgres psql -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM scanned_windows;" 2>/dev/null | tr -d '[:space:]')
    FOUND_COUNT=$(docker exec puzzle_postgres psql -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM found_keys;" 2>/dev/null | tr -d '[:space:]')
else
    PUZZLE_COUNT=$(PGPASSWORD=postgrespassword psql -h localhost -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM puzzles;" 2>/dev/null | tr -d '[:space:]' || sudo -u postgres psql -d puzzle_hunt -t -c "SELECT count(*) FROM puzzles;" | tr -d '[:space:]')
    WINDOWS_COUNT=$(PGPASSWORD=postgrespassword psql -h localhost -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM scanned_windows;" 2>/dev/null | tr -d '[:space:]' || sudo -u postgres psql -d puzzle_hunt -t -c "SELECT count(*) FROM scanned_windows;" | tr -d '[:space:]')
    FOUND_COUNT=$(PGPASSWORD=postgrespassword psql -h localhost -U postgres -d puzzle_hunt -t -c "SELECT count(*) FROM found_keys;" 2>/dev/null | tr -d '[:space:]' || sudo -u postgres psql -d puzzle_hunt -t -c "SELECT count(*) FROM found_keys;" | tr -d '[:space:]')
fi

echo -e "   📋 Puzzles terdaftar      : ${GREEN}$PUZZLE_COUNT${NC}"
echo -e "   🔍 Scanned Windows riwayat : ${GREEN}$WINDOWS_COUNT${NC}"
echo -e "   🔑 Kunci ditemukan         : ${GREEN}$FOUND_COUNT${NC}"

echo ""
echo -e "${GREEN}🎉 IMPORT DATABASE BERHASIL! Seluruh progres lokal telah tersinkronisasi ke cloud.${NC}"
echo -e "${CYAN}======================================================================${NC}"

