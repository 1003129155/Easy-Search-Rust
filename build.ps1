#!/usr/bin/env pwsh
<#
.SYNOPSIS
    EasySearch 一键编译脚本
.DESCRIPTION
    自动完成：停止运行中的进程 → 编译 release → 报告结果
.PARAMETER Clean
    加 -Clean 会先 cargo clean 再编译（完全重编）
.PARAMETER Package
    指定要编译的 package，如 -Package easysearch 只编译主程序
    不指定则编译整个 workspace
.EXAMPLE
    .\build.ps1              # 增量编译整个 workspace
    .\build.ps1 -Clean       # 清理后全量编译
    .\build.ps1 -Package easysearch   # 只编译主程序
#>

param(
    [switch]$Clean,
    [string]$Package
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# ── 颜色输出辅助 ──
function Write-Step($msg) { Write-Host "[*] $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "[✓] $msg" -ForegroundColor Green }
function Write-Err($msg)  { Write-Host "[✗] $msg" -ForegroundColor Red }

# ── 项目根目录（脚本所在目录）──
$ProjectRoot = $PSScriptRoot
Push-Location $ProjectRoot

try {
    # ── 1. 停止正在运行的进程 ──
    Write-Step "停止运行中的 EasySearch 进程..."
    $proc = Get-Process -Name "easysearch" -ErrorAction SilentlyContinue
    if ($proc) {
        Stop-Process -Name "easysearch" -Force
        Write-Host "    已停止: easysearch.exe (PID: $($proc.Id -join ', '))"
        Start-Sleep -Seconds 1
    }

    # ── 2. 可选清理 ──
    if ($Clean) {
        Write-Step "执行 cargo clean..."
        cargo clean
        if ($LASTEXITCODE -ne 0) {
            Write-Err "cargo clean 失败"
            exit 1
        }
    }

    # ── 3. 编译 ──
    $buildArgs = @("build", "--release")
    if ($Package) {
        $buildArgs += @("-p", $Package)
        Write-Step "编译 package: $Package (release)..."
    } else {
        # 默认只编译主程序（唯一产物）
        $buildArgs += @("-p", "easysearch")
        Write-Step "编译 easysearch (release)..."
    }

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    cargo @buildArgs
    $sw.Stop()

    if ($LASTEXITCODE -ne 0) {
        Write-Err "编译失败！(耗时 $([math]::Round($sw.Elapsed.TotalSeconds, 1))s)"
        exit 1
    }

    # ── 4. 报告结果 ──
    Write-Ok "编译成功！(耗时 $([math]::Round($sw.Elapsed.TotalSeconds, 1))s)"
    Write-Host ""
    Write-Host "产物目录: $ProjectRoot\target\release\" -ForegroundColor Yellow
    Write-Host ""

    # 列出产物
    $exe = Get-Item "$ProjectRoot\target\release\easysearch.exe" -ErrorAction SilentlyContinue
    if ($exe) {
        $size = "{0:N2} MB" -f ($exe.Length / 1MB)
        $time = $exe.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss")
        Write-Host "  easysearch.exe  $size  $time" -ForegroundColor White
    }

} finally {
    Pop-Location
}
