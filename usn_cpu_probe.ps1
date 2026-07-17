# USN polling CPU benchmark sampler (ASCII-only to avoid PS encoding issues).
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File usn_cpu_probe.ps1 -Label before
#   powershell -ExecutionPolicy Bypass -File usn_cpu_probe.ps1 -Label after
#
# Steps:
#   1. Launch poll_probe example (prebuilt release binary).
#   2. Wait for "POLL_PHASE_START <pid>" marker, record start CPU time + wall clock.
#   3. Generate fixed-rate USN load during the window (create/delete temp files).
#   4. Wait for "POLL_PHASE_END", record end CPU time + wall clock.
#   5. Compute CPU seconds consumed and avg CPU% (relative to one core).
#
# Both before/after runs use the identical load script for comparability.

param(
    [string]$Label = "run",
    [int]$WindowSecs = 60,
    [string]$Drives = "C",
    [int]$LoadFilesPerBurst = 200,
    [int]$BurstIntervalMs = 500,
    # When set, probe uses the OLD subscribe-to-everything mask (0xFFFFFFFF).
    [switch]$FullMask,
    # Load pattern: "overwrite" (rewrite existing files -> DATA_OVERWRITE/CLOSE
    # noise, the reason types the narrowed mask filters out) or "createdelete"
    # (create+delete -> reasons the narrowed mask keeps).
    [ValidateSet("overwrite", "createdelete")]
    [string]$Load = "overwrite"
)

$ErrorActionPreference = "Stop"
$repo = $PSScriptRoot
$exe = Join-Path $repo "target\release\examples\poll_probe.exe"

if (-not (Test-Path $exe)) {
    Write-Error "poll_probe.exe not found. Build first: cargo build --release -p easysearch-engine --example poll_probe"
    exit 1
}

$loadDir = Join-Path $env:TEMP "easysearch_usn_load"
if (Test-Path $loadDir) { Remove-Item $loadDir -Recurse -Force }
New-Item -ItemType Directory -Path $loadDir | Out-Null

Write-Host "[sampler] label=$Label window=${WindowSecs}s drives=$Drives load=$LoadFilesPerBurst/burst @${BurstIntervalMs}ms"

$env:EASYSEARCH_DRIVES = $Drives
$env:EASYSEARCH_PROBE_SECS = "$WindowSecs"
if ($FullMask) {
    $env:EASYSEARCH_USN_FULL_MASK = "1"
    Write-Host "[sampler] mask=FULL(0xFFFFFFFF) load=$Load"
} else {
    Remove-Item Env:\EASYSEARCH_USN_FULL_MASK -ErrorAction SilentlyContinue
    Write-Host "[sampler] mask=NARROW load=$Load"
}

$outFile = Join-Path $loadDir "probe_stdout.txt"
$errFile = Join-Path $loadDir "probe_stderr.txt"
$proc = Start-Process -FilePath $exe -RedirectStandardOutput $outFile -RedirectStandardError $errFile -PassThru -NoNewWindow

# Wait for POLL_PHASE_START marker
$probePid = $null
$deadline = (Get-Date).AddSeconds(180)
while ((Get-Date) -lt $deadline) {
    if (Test-Path $outFile) {
        $line = Select-String -Path $outFile -Pattern "POLL_PHASE_START (\d+)" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($line) {
            $probePid = [int]$line.Matches[0].Groups[1].Value
            break
        }
    }
    Start-Sleep -Milliseconds 200
}

if (-not $probePid) {
    Write-Error "POLL_PHASE_START marker not detected; probe may have failed. stderr:"
    if (Test-Path $errFile) { Get-Content $errFile | Write-Host }
    $proc | Stop-Process -Force -ErrorAction SilentlyContinue
    exit 1
}

$p = Get-Process -Id $probePid
$cpuStart = $p.TotalProcessorTime.TotalSeconds
$wallStart = Get-Date
Write-Host "[sampler] window START pid=$probePid cpuStart=$([math]::Round($cpuStart,3))s"

# Generate fixed-rate USN load
$loadEnd = $wallStart.AddSeconds($WindowSecs - 2)
$counter = 0

if ($Load -eq "overwrite") {
    # Pre-create a fixed set of files ONCE, then repeatedly rewrite them.
    # Rewriting an existing file emits DATA_OVERWRITE/DATA_EXTEND/CLOSE USN
    # records (NOT create/delete). This is the everyday "write noise" that the
    # narrowed mask filters at the kernel boundary, so it isolates the change.
    $files = @()
    for ($i = 0; $i -lt $LoadFilesPerBurst; $i++) {
        $f = Join-Path $loadDir "ow_$i.tmp"
        Set-Content -Path $f -Value "init" -NoNewline
        $files += $f
    }
    while ((Get-Date) -lt $loadEnd) {
        foreach ($f in $files) {
            Set-Content -Path $f -Value "usn overwrite payload $counter" -NoNewline
        }
        $counter++
        Start-Sleep -Milliseconds $BurstIntervalMs
    }
} else {
    while ((Get-Date) -lt $loadEnd) {
        for ($i = 0; $i -lt $LoadFilesPerBurst; $i++) {
            $f = Join-Path $loadDir "load_$($counter)_$i.tmp"
            Set-Content -Path $f -Value "usn load payload line" -NoNewline
        }
        Get-ChildItem $loadDir -Filter "load_${counter}_*.tmp" | Remove-Item -Force
        $counter++
        Start-Sleep -Milliseconds $BurstIntervalMs
    }
}

# Wait for POLL_PHASE_END (probe stays alive ~6s afterwards for a clean read)
$deadline = (Get-Date).AddSeconds(30)
while ((Get-Date) -lt $deadline) {
    if (Select-String -Path $outFile -Pattern "POLL_PHASE_END" -ErrorAction SilentlyContinue) { break }
    Start-Sleep -Milliseconds 200
}

# Capture end CPU immediately while the probe is guaranteed still alive.
$p.Refresh()
if ($p.HasExited) {
    Write-Error "probe exited before end CPU could be sampled; measurement invalid"
    exit 1
}
$cpuEnd = $p.TotalProcessorTime.TotalSeconds
$wallEnd = Get-Date

$proc | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue

$cpuDelta = $cpuEnd - $cpuStart
$wallDelta = ($wallEnd - $wallStart).TotalSeconds
$avgPct = if ($wallDelta -gt 0) { ($cpuDelta / $wallDelta) * 100 } else { 0 }

Write-Host ""
Write-Host "======== RESULT [$Label] ========"
Write-Host ("  wall window:  {0:N2} s" -f $wallDelta)
Write-Host ("  cpu consumed: {0:N3} s" -f $cpuDelta)
Write-Host ("  avg CPU:      {0:N2} pct (of one core)" -f $avgPct)
Write-Host ("  load bursts:  {0}" -f $counter)
Write-Host "================================="

$resultFile = Join-Path $repo "usn_cpu_results.txt"
$stamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
$maskLabel = if ($FullMask) { "FULL" } else { "NARROW" }
Add-Content -Path $resultFile -Value ("[{0}] label={1} mask={2} loadType={3} window={4:N2}s cpu={5:N3}s avg={6:N2}pct bursts={7} drives={8} files={9}/{10}ms" -f $stamp, $Label, $maskLabel, $Load, $wallDelta, $cpuDelta, $avgPct, $counter, $Drives, $LoadFilesPerBurst, $BurstIntervalMs)

Remove-Item $loadDir -Recurse -Force -ErrorAction SilentlyContinue
Write-Host "[sampler] result appended to usn_cpu_results.txt"
