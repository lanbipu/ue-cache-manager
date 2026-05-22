@echo off
REM UECM WinRM Bootstrap — one-click entry point.
REM Double-click this file. If not running elevated, it relaunches itself
REM with UAC; once elevated it runs UECM-Bootstrap-WinRM.ps1 with all the
REM switches UECM expects.

REM Admin check via fltmc.exe — native Windows tool that needs admin token but
REM does NOT depend on the Server / LanmanServer service. (NET SESSION would
REM falsely report non-zero when LanmanServer is stopped, which is exactly the
REM state -EnableSmbServer is meant to fix → that would cause an infinite UAC
REM relaunch loop.)
fltmc >nul 2>&1
if %errorlevel% NEQ 0 (
    echo Requesting administrator privileges...
    powershell.exe -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

REM Force UTF-8 console so the PowerShell script's JSON / Chinese log lines render correctly.
chcp 65001 >nul

setlocal
set "SCRIPT_DIR=%~dp0"
set "PS1=%SCRIPT_DIR%UECM-Bootstrap-WinRM.ps1"

if not exist "%PS1%" (
    echo.
    echo [ERROR] UECM-Bootstrap-WinRM.ps1 not found next to this .cmd file.
    echo Expected at: %PS1%
    echo.
    pause
    exit /b 1
)

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%PS1%" ^
    -NetworkCategory Private ^
    -EnableLocalAccountRemoteAdmin ^
    -EnableSmbServer ^
    -EnableWmi ^
    -SetExecutionPolicy RemoteSigned ^
    -EnableLongPaths ^
    -PowerProfile HighPerformance

set "PS_EXIT=%ERRORLEVEL%"

echo.
echo ============================================
if "%PS_EXIT%"=="0" (
    echo  UECM Bootstrap done. You can close this window.
) else (
    echo  UECM Bootstrap failed with exit code %PS_EXIT%.
    echo  Check the JSON message above for details.
)
echo ============================================
echo.
REM Auto-close so unattended / scripted runs don't hang on a key press.
REM A real key press still closes it immediately; no /nobreak on purpose.
REM 2>nul: if stdin is redirected (non-interactive) timeout errors out and we
REM just fall through to exit instead of blocking like pause did.
echo This window auto-closes in 20s. Press any key to close now...
timeout /t 20 >nul 2>nul
exit /b %PS_EXIT%
