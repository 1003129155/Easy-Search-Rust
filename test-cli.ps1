# test-cli.ps1 - EasySearch daemon diagnostic
# Usage: powershell -ExecutionPolicy Bypass -File test-cli.ps1

$ErrorActionPreference = "Continue"
$exe = "$PSScriptRoot\target\release\easysearch.exe"

Write-Host "=== EasySearch Diagnostic ===" -ForegroundColor Cyan
Write-Host ""

# 1. Check exe
if (!(Test-Path $exe)) {
    Write-Host "[ERROR] exe not found: $exe" -ForegroundColor Red
    exit 1
}
$size = [math]::Round((Get-Item $exe).Length / 1MB, 2)
Write-Host "[OK] exe exists ($size MB)" -ForegroundColor Green
Write-Host ""

# 2. Check daemon process
$procs = Get-Process -Name "easysearch" -ErrorAction SilentlyContinue
if ($procs) {
    $mem = [math]::Round(($procs | Measure-Object WorkingSet64 -Sum).Sum / 1MB, 1)
    Write-Host "[INFO] daemon running (PID: $($procs.Id -join ', '), Mem: $mem MB)" -ForegroundColor Yellow
} else {
    Write-Host "[INFO] daemon not running, starting..." -ForegroundColor Yellow
    Start-Process -FilePath $exe -ArgumentList "--daemon" -WindowStyle Hidden
    Start-Sleep -Seconds 3
    $procs = Get-Process -Name "easysearch" -ErrorAction SilentlyContinue
    if ($procs) {
        Write-Host "[OK] daemon started (PID: $($procs.Id -join ', '))" -ForegroundColor Green
    } else {
        Write-Host "[ERROR] daemon failed to start" -ForegroundColor Red
    }
}
Write-Host ""

# 3. Find pipe
Write-Host "--- Pipe Discovery ---" -ForegroundColor Cyan
$pipes = Get-ChildItem "\\.\pipe\" -ErrorAction SilentlyContinue | Where-Object { $_.Name -match "easysearch|uffs" }
if ($pipes) {
    foreach ($p in $pipes) {
        Write-Host "  Found pipe: \\.\pipe\$($p.Name)" -ForegroundColor Green
    }
    $targetPipe = $pipes[0].Name
} else {
    Write-Host "  No easysearch/uffs pipe found!" -ForegroundColor Red
    Write-Host "  Listing all pipes with 'easy' or 'uffs'..."
    $alt = Get-ChildItem "\\.\pipe\" -ErrorAction SilentlyContinue | Where-Object { $_.Name -match "easy|uffs" }
    if ($alt) {
        foreach ($p in $alt) { Write-Host "  $($p.Name)" -ForegroundColor Yellow }
    } else {
        Write-Host "  None found." -ForegroundColor Red
    }
    Write-Host ""
    Write-Host "  Daemon stderr (if any) may show why index failed."
    Write-Host "  Try running daemon directly: $exe --daemon"
    exit 1
}
Write-Host ""

# 4. Connect and send status
Write-Host "--- Pipe Connection Test ---" -ForegroundColor Cyan
try {
    $client = New-Object System.IO.Pipes.NamedPipeClientStream(".", $targetPipe, [System.IO.Pipes.PipeDirection]::InOut)
    $client.Connect(5000)
    Write-Host "[OK] Connected to pipe!" -ForegroundColor Green

    $sw = New-Object System.IO.StreamWriter($client)
    $sr = New-Object System.IO.StreamReader($client)
    $sw.AutoFlush = $true

    # Send status
    $req = '{"id":0,"method":"status"}'
    Write-Host "  >> $req"
    $sw.WriteLine($req)
    $resp = $sr.ReadLine()
    Write-Host "  << $resp" -ForegroundColor White

    $obj = $resp | ConvertFrom-Json
    Write-Host ""
    Write-Host "  ready:    $($obj.ready)"
    Write-Host "  indexing: $($obj.indexing)"
    Write-Host "  drives:   $($obj.drives -join ', ')"
    Write-Host "  records:  $($obj.records)"
    Write-Host ""

    # If ready, do search
    if ($obj.ready -eq $true) {
        Write-Host "--- Search Test: .txt (limit 5) ---" -ForegroundColor Cyan
        $searchReq = '{"id":1,"method":"search","query":".txt","limit":5}'
        Write-Host "  >> $searchReq"
        $sw.WriteLine($searchReq)
        $searchResp = $sr.ReadLine()

        $sObj = $searchResp | ConvertFrom-Json
        if ($sObj.items -and $sObj.items.Count -gt 0) {
            Write-Host "[OK] Got $($sObj.items.Count) results:" -ForegroundColor Green
            foreach ($item in $sObj.items) {
                $t = if ($item.is_directory) {"[DIR] "} else {"[FILE]"}
                Write-Host "  $t $($item.path)"
            }
        } else {
            Write-Host "[WARN] 0 results returned" -ForegroundColor Yellow
            Write-Host "  Raw: $searchResp"
        }
    } else {
        Write-Host "[WAIT] Index not ready yet. Waiting 15s..." -ForegroundColor Yellow
        Start-Sleep -Seconds 15
        $sw.WriteLine($req)
        $resp2 = $sr.ReadLine()
        Write-Host "  Retry: $resp2"
    }

    $client.Close()
} catch {
    Write-Host "[ERROR] $($_.Exception.Message)" -ForegroundColor Red
}

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Cyan
