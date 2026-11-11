@echo off
setlocal enabledelayedexpansion

echo ======================================================================
echo    📦 BITCOIN PUZZLE HUNTER — EXPORT DATABASE POSTGRESQL LOKAL
echo ======================================================================
echo.

cd /d "%~dp0"

set OUTPUT_DIR=data
if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"
set BACKUP_FILE=%OUTPUT_DIR%\puzzle_hunt_backup.sql

echo [*] Memeriksa container PostgreSQL (puzzle_postgres)...
docker ps --format "{{.Names}}" | findstr /I "puzzle_postgres" > nul
if %errorlevel% neq 0 (
    echo [!] ERROR: Container 'puzzle_postgres' tidak berjalan!
    echo     Jalankan Docker Desktop dan pastikan database aktif:
    echo     docker compose up -d postgres
    exit /b 1
)

echo [*] Mengekspor database puzzle_hunt ke %BACKUP_FILE%...
docker exec puzzle_postgres pg_dump -U postgres -d puzzle_hunt --clean --if-exists > "%BACKUP_FILE%"

if %errorlevel% equ 0 (
    echo.
    echo [OK] Export berhasil!
    for %%F in ("%BACKUP_FILE%") do echo      Ukuran file: %%~zF bytes
    echo      Lokasi     : %BACKUP_FILE%
    echo.
    echo [>] Siap dikirim ke Cloud / GPU Hosting:
    echo     Kirim file ini ke server GPU di folder data/
    echo     Lalu jalankan di server GPU: bash import_db.sh
) else (
    echo.
    echo [!] ERROR: Gagal mengekspor database.
    exit /b 1
)

echo ======================================================================
