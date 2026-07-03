@echo off
REM ============================================================
REM dev.bat - ビルド後にプラグインディレクトリへ自動コピー
REM 使い方: dev.bat [release|debug]
REM   デフォルトは release ビルド
REM ============================================================

setlocal enabledelayedexpansion

REM --- 設定 ---
set "PLUGIN_ENGINE_DIR=%~dp0..\Flow.Launcher-dev\Plugins\Flow.Launcher.Plugin.Explorer\Engine"
set "PROFILE=%~1"
if "%PROFILE%"=="" set "PROFILE=release"

REM --- Rust ツールチェインの PATH 設定 ---
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

REM --- VS 開発者環境の初期化 ---
if exist "C:\Program Files\Microsoft Visual Studio\18\Professional\VC\Auxiliary\Build\vcvars64.bat" (
    call "C:\Program Files\Microsoft Visual Studio\18\Professional\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
) else if exist "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat" (
    call "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
) else if exist "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" (
    call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
)

REM --- 証明書チェックを無効化（社内環境向け） ---
set CARGO_HTTP_CHECK_REVOKE=false

REM --- ビルド実行 ---
echo [1/2] Building easysearch (%PROFILE%)...

if "%PROFILE%"=="release" (
    cargo build --release -p easysearch
) else (
    cargo build -p easysearch
)

if %ERRORLEVEL% neq 0 (
    echo [ERROR] ビルド失敗
    exit /b %ERRORLEVEL%
)

REM --- 出力先ディレクトリの確認 ---
if not exist "%PLUGIN_ENGINE_DIR%" (
    echo [ERROR] プラグインディレクトリが見つかりません: %PLUGIN_ENGINE_DIR%
    exit /b 1
)

REM --- コピー実行 ---
echo [2/2] Copying to plugin directory...

if "%PROFILE%"=="release" (
    set "BUILD_DIR=%~dp0target\release"
) else (
    set "BUILD_DIR=%~dp0target\debug"
)

if exist "!BUILD_DIR!\easysearch.exe" (
    copy /Y "!BUILD_DIR!\easysearch.exe" "%PLUGIN_ENGINE_DIR%\easysearch.exe" >nul
    echo   easysearch.exe -^> Engine\
) else (
    echo [WARN] easysearch.exe not found in !BUILD_DIR!
)

echo.
echo [DONE] プラグインディレクトリに反映済み: %PLUGIN_ENGINE_DIR%
