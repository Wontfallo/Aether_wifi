@echo off
REM ═══════════════════════════════════════════════
REM  AETHER WiFi Auditor — Windows Launcher
REM  Double-click this to launch Aether
REM ═══════════════════════════════════════════════
title Aether WiFi Auditor
echo.
echo   ◈ AETHER WiFi Auditor ◈
echo   Launching via Kali WSL2...
echo.
Pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\attach_wsl_wifi.ps1" -AdapterName "802.11"
if errorlevel 1 (
  echo.
  echo [ERROR] Failed to attach the WiFi adapter to WSL.
  pause
  exit /b 1
)
wsl -d kali-linux -- bash /mnt/c/Users/WontML/dev/Aether_wifi/aether.sh
pause
