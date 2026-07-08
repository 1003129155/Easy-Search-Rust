#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Generate EasySearch app icon (ICO) — black & white magnifying glass.
.DESCRIPTION
    Creates a multi-resolution .ico file with 16x16, 32x32, 48x48, 64x64, and 256x256 images.
    The design: a white magnifying glass (circle + handle) on a transparent background,
    similar to a search icon. Pure black stroke, white fill, clean look.
#>

Add-Type -AssemblyName System.Drawing

$outputPath = Join-Path $PSScriptRoot "assets\app.ico"

# Ensure output directory exists
$dir = Split-Path $outputPath -Parent
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }

function Draw-SearchIcon([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    # Scale factor
    $s = $size / 64.0

    # Magnifying glass parameters (designed for 64x64 canvas)
    $circleX = 16 * $s
    $circleY = 12 * $s
    $circleDia = 30 * $s
    $strokeWidth = [Math]::Max(2, 4 * $s)
    $handleWidth = [Math]::Max(2, 5 * $s)

    # Circle center for handle calculation
    $cx = $circleX + $circleDia / 2
    $cy = $circleY + $circleDia / 2
    $r = $circleDia / 2

    # Handle endpoint (45 degrees down-right from circle edge)
    $angle = [Math]::PI / 4  # 45 degrees
    $handleStartX = $cx + $r * [Math]::Cos($angle)
    $handleStartY = $cy + $r * [Math]::Sin($angle)
    $handleLen = 16 * $s
    $handleEndX = $handleStartX + $handleLen * [Math]::Cos($angle)
    $handleEndY = $handleStartY + $handleLen * [Math]::Sin($angle)

    # Draw white filled circle with black border
    $whiteBrush = [System.Drawing.Brushes]::White
    $blackPen = New-Object System.Drawing.Pen([System.Drawing.Color]::Black, $strokeWidth)
    $blackPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $blackPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round

    # Fill circle white
    $g.FillEllipse($whiteBrush, $circleX, $circleY, $circleDia, $circleDia)
    # Draw circle border
    $g.DrawEllipse($blackPen, $circleX, $circleY, $circleDia, $circleDia)

    # Draw handle
    $handlePen = New-Object System.Drawing.Pen([System.Drawing.Color]::Black, $handleWidth)
    $handlePen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $handlePen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.DrawLine($handlePen, [float]$handleStartX, [float]$handleStartY, [float]$handleEndX, [float]$handleEndY)

    $g.Dispose()
    $blackPen.Dispose()
    $handlePen.Dispose()
    return $bmp
}

function Write-ICO([System.Drawing.Bitmap[]]$images, [string]$path) {
    # ICO file format:
    # Header (6 bytes) + Directory entries (16 bytes each) + Image data (PNG)
    $ms = New-Object System.IO.MemoryStream

    $writer = New-Object System.IO.BinaryWriter($ms)

    # ICO header
    $writer.Write([UInt16]0)           # Reserved
    $writer.Write([UInt16]1)           # Type: 1 = ICO
    $writer.Write([UInt16]$images.Count) # Number of images

    # We'll store PNG data for each image
    $pngData = @()
    foreach ($img in $images) {
        $pngStream = New-Object System.IO.MemoryStream
        $img.Save($pngStream, [System.Drawing.Imaging.ImageFormat]::Png)
        $pngData += ,$pngStream.ToArray()
        $pngStream.Dispose()
    }

    # Calculate offsets
    $headerSize = 6
    $dirSize = 16 * $images.Count
    $dataOffset = $headerSize + $dirSize

    # Write directory entries
    for ($i = 0; $i -lt $images.Count; $i++) {
        $img = $images[$i]
        $data = $pngData[$i]
        $w = if ($img.Width -ge 256) { 0 } else { $img.Width }
        $h = if ($img.Height -ge 256) { 0 } else { $img.Height }

        $writer.Write([byte]$w)        # Width
        $writer.Write([byte]$h)        # Height
        $writer.Write([byte]0)         # Color palette
        $writer.Write([byte]0)         # Reserved
        $writer.Write([UInt16]1)       # Color planes
        $writer.Write([UInt16]32)      # Bits per pixel
        $writer.Write([UInt32]$data.Length) # Size of image data
        $writer.Write([UInt32]$dataOffset)  # Offset to image data

        $dataOffset += $data.Length
    }

    # Write image data
    foreach ($data in $pngData) {
        $writer.Write($data)
    }

    $writer.Flush()
    [System.IO.File]::WriteAllBytes($path, $ms.ToArray())
    $writer.Dispose()
    $ms.Dispose()
}

# Generate images at multiple resolutions
$sizes = @(16, 32, 48, 64, 256)
$bitmaps = @()
foreach ($sz in $sizes) {
    $bitmaps += Draw-SearchIcon $sz
}

Write-ICO $bitmaps $outputPath

# Cleanup
foreach ($bmp in $bitmaps) { $bmp.Dispose() }

Write-Host "[OK] Icon generated: $outputPath" -ForegroundColor Green
Write-Host "     Sizes: $($sizes -join ', ')px"
