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
wsl -d kali-linux -- bash /mnt/c/Users/WontML/dev/Aether_wifi/aether.sh
pause
