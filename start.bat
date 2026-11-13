@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul 2>&1
title Bitcoin Puzzle Hunter - Launcher
color 0A

echo.
echo  ================================================================
echo    BITCOIN PUZZLE HUNTER - LAUNCHER
echo  ================================================================
echo    Backend  : Rust (Axum) - http://localhost:3001
echo    Frontend : Vite (React) - http://localhost:5173
echo    Database : PostgreSQL   - localhost:5432
echo  ================================================================
echo.

:: Move to project root
cd /d "%~dp0"

:: ----------------------------------------------------------------
:: 1. Load .env file
:: ----------------------------------------------------------------
if exist ".env" (
    echo [ENV] Loading .env file...
    for /f "usebackq tokens=1,* delims==" %%A in (".env") do (
        set "firstchar=%%A"
        set "firstchar=!firstchar:~0,1!"
        if not "!firstchar!"=="#" (
            if not "%%A"=="" (
                set "%%A=%%B" 2>nul
            )
        )
    )
    echo [ENV] HUNT_MODE = !HUNT_MODE!
) else (
    echo [ENV] WARNING: .env file not found, using defaults
)

:: ----------------------------------------------------------------
:: 2. Check and Start PostgreSQL (Docker)
:: ----------------------------------------------------------------
echo.
echo [DB] Checking PostgreSQL...

where docker >nul 2>&1
if !errorlevel! neq 0 (
    echo [DB] WARNING: Docker not found! Make sure PostgreSQL is running manually.
    goto :skip_docker
)

docker ps --filter "name=puzzle_postgres" --format "{{.Names}}" 2>nul | findstr /i "puzzle_postgres" >nul 2>&1
if !errorlevel! equ 0 (
    echo [DB] OK - PostgreSQL container already running
) else (
    echo [DB] Starting PostgreSQL container...
    docker compose up -d postgres
    if !errorlevel! equ 0 (
        echo [DB] OK - PostgreSQL started
        echo [DB] Waiting 3 seconds for DB to be ready...
        timeout /t 3 /nobreak >nul
    ) else (
        echo [DB] WARNING: Failed to start PostgreSQL. Check Docker.
    )
)

:skip_docker

:: ----------------------------------------------------------------
:: 3. Kill previous instances (avoid port conflicts)
:: ----------------------------------------------------------------
echo.
echo [CLEANUP] Stopping previous instances...

tasklist /fi "imagename eq puzzle-hunt.exe" 2>nul | findstr /i "puzzle-hunt" >nul 2>&1
if !errorlevel! equ 0 (
    echo [CLEANUP] Stopping old puzzle-hunt.exe...
    taskkill /f /im "puzzle-hunt.exe" >nul 2>&1
    timeout /t 1 /nobreak >nul
)

tasklist /fi "imagename eq puzzle-hunt-old.exe" 2>nul | findstr /i "puzzle-hunt-old" >nul 2>&1
if !errorlevel! equ 0 (
    echo [CLEANUP] Stopping old puzzle-hunt-old.exe...
    taskkill /f /im "puzzle-hunt-old.exe" >nul 2>&1
)

for /f "tokens=5" %%p in ('netstat -aon ^| findstr ":5173" ^| findstr "LISTENING" 2^>nul') do (
    echo [CLEANUP] Stopping process on port 5173 PID: %%p
    taskkill /f /pid %%p >nul 2>&1
)

echo [CLEANUP] Done

:: ----------------------------------------------------------------
:: 4. Check binary exists
:: ----------------------------------------------------------------
echo.
if not exist "target\release\puzzle-hunt.exe" (
    echo [ERROR] Backend binary not found!
    echo [ERROR] Run: cargo build --release
    echo.
    pause
    exit /b 1
)
echo [BINARY] OK - target\release\puzzle-hunt.exe found

:: ----------------------------------------------------------------
:: 6. Start Backend (in new window)
:: ----------------------------------------------------------------
echo.
echo [BACKEND] Starting Rust backend server...
start "Puzzle Hunt - Backend" /min cmd /c "cd /d "%~dp0" && target\release\puzzle-hunt.exe"
timeout /t 2 /nobreak >nul
echo [BACKEND] OK - Running on http://localhost:3001


echo [INFO] Tutup window ini TIDAK akan menghentikan backend/frontend.
echo [INFO] Gunakan stop.bat untuk menghentikan semua service.
echo.
pause
endlocal
