@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul 2>&1
title Bitcoin Puzzle Hunter - Stop All Services
color 0C

echo.
echo  ================================================================
echo    BITCOIN PUZZLE HUNTER - STOP ALL SERVICES
echo  ================================================================
echo.

:: ----------------------------------------------------------------
:: 1. Stop Backend
:: ----------------------------------------------------------------
echo [BACKEND] Stopping puzzle-hunt.exe...
tasklist /fi "imagename eq puzzle-hunt.exe" 2>nul | findstr /i "puzzle-hunt" >nul 2>&1
if !errorlevel! equ 0 (
    taskkill /f /im "puzzle-hunt.exe" >nul 2>&1
    echo [BACKEND] OK - Stopped
) else (
    echo [BACKEND] -- Not running
)

tasklist /fi "imagename eq puzzle-hunt-old.exe" 2>nul | findstr /i "puzzle-hunt-old" >nul 2>&1
if !errorlevel! equ 0 (
    taskkill /f /im "puzzle-hunt-old.exe" >nul 2>&1
    echo [BACKEND] OK - Old binary stopped
)

:: ----------------------------------------------------------------
:: 2. Stop Frontend (Vite on port 5173)
:: ----------------------------------------------------------------
echo.
echo [FRONTEND] Stopping Vite dev server (port 5173)...
set "found_vite=0"
for /f "tokens=5" %%p in ('netstat -aon ^| findstr ":5173" ^| findstr "LISTENING" 2^>nul') do (
    taskkill /f /pid %%p >nul 2>&1
    set "found_vite=1"
)
if "!found_vite!"=="1" (
    echo [FRONTEND] OK - Stopped
) else (
    echo [FRONTEND] -- Not running
)

:: ----------------------------------------------------------------
:: 3. Stop PostgreSQL Docker (optional)
:: ----------------------------------------------------------------
echo.
set /p stopdb="[DB] Stop PostgreSQL container juga? (y/N): "
if /i "!stopdb!"=="y" (
    cd /d "%~dp0"
    docker compose down
    echo [DB] OK - PostgreSQL stopped
) else (
    echo [DB] -- PostgreSQL tetap berjalan
)

:: ----------------------------------------------------------------
:: Done
:: ----------------------------------------------------------------
echo.
echo  ================================================================
echo    ALL SERVICES STOPPED
echo  ================================================================
echo.
pause
endlocal
