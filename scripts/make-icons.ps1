$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

function New-RoundedRect([int]$x, [int]$y, [int]$w, [int]$h, [int]$r) {
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = $r * 2
  $path.AddArc($x, $y, $d, $d, 180, 90)
  $path.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
  $path.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
  $path.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
  $path.CloseFigure()
  return $path
}

function New-Mark([int]$size, [System.Drawing.Color]$top, [System.Drawing.Color]$bottom, [string]$letter) {
  $bmp = New-Object System.Drawing.Bitmap $size, $size
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
  $g.Clear([System.Drawing.Color]::Transparent)
  $pad = [Math]::Max(2, [int]($size * 0.06))
  $radius = [Math]::Max(8, [int]($size * 0.22))
  $path = New-RoundedRect $pad $pad ($size - 2 * $pad) ($size - 2 * $pad) $radius
  $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush (
    (New-Object System.Drawing.Point 0, 0),
    (New-Object System.Drawing.Point 0, $size),
    $top,
    $bottom
  )
  $g.FillPath($brush, $path)
  $fontSize = [int]($size * $(if ($letter.Length -gt 1) { 0.38 } else { 0.48 }))
  $font = New-Object System.Drawing.Font "Segoe UI", $fontSize, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
  $sf = New-Object System.Drawing.StringFormat
  $sf.Alignment = [System.Drawing.StringAlignment]::Center
  $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
  $rect = New-Object System.Drawing.RectangleF 0, ([int]($size * 0.02)), $size, $size
  $g.DrawString($letter, $font, [System.Drawing.Brushes]::White, $rect, $sf)
  $font.Dispose()
  $brush.Dispose()
  $path.Dispose()
  $g.Dispose()
  return $bmp
}

function Get-Dib32([System.Drawing.Bitmap]$bmp) {
  $w = $bmp.Width
  $h = $bmp.Height
  $xorStride = $w * 4
  $andStride = [Math]::Ceiling($w / 32.0) * 4
  $header = New-Object byte[] 40
  $bw = $w
  $bh = $h * 2
  [BitConverter]::GetBytes([int]40).CopyTo($header, 0)
  [BitConverter]::GetBytes([int]$bw).CopyTo($header, 4)
  [BitConverter]::GetBytes([int]$bh).CopyTo($header, 8)
  [BitConverter]::GetBytes([int16]1).CopyTo($header, 12)
  [BitConverter]::GetBytes([int16]32).CopyTo($header, 14)
  $xor = New-Object byte[] ($xorStride * $h)
  $and = New-Object byte[] ($andStride * $h)
  for ($y = 0; $y -lt $h; $y++) {
    $row = ($h - 1 - $y) * $xorStride
    for ($x = 0; $x -lt $w; $x++) {
      $c = $bmp.GetPixel($x, $y)
      $i = $row + $x * 4
      $xor[$i] = $c.B
      $xor[$i + 1] = $c.G
      $xor[$i + 2] = $c.R
      $xor[$i + 3] = $c.A
    }
  }
  $out = New-Object byte[] ($header.Length + $xor.Length + $and.Length)
  [Buffer]::BlockCopy($header, 0, $out, 0, $header.Length)
  [Buffer]::BlockCopy($xor, 0, $out, $header.Length, $xor.Length)
  [Buffer]::BlockCopy($and, 0, $out, $header.Length + $xor.Length, $and.Length)
  return $out
}

function Save-Ico([System.Drawing.Bitmap[]]$images, [string]$path) {
  $blobs = New-Object "System.Collections.Generic.List[byte[]]"
  foreach ($img in $images) {
    $dib = Get-Dib32 $img
    if ($dib -is [object[]]) { $dib = [byte[]]$dib }
    $blobs.Add([byte[]]$dib)
  }
  $count = $images.Length
  $offset = 6 + 16 * $count
  $ms = New-Object System.IO.MemoryStream
  $bw = New-Object System.IO.BinaryWriter $ms
  $bw.Write([uint16]0)
  $bw.Write([uint16]1)
  $bw.Write([uint16]$count)
  for ($i = 0; $i -lt $count; $i++) {
    $img = $images[$i]
    $w = if ($img.Width -ge 256) { 0 } else { [byte]$img.Width }
    $h = if ($img.Height -ge 256) { 0 } else { [byte]$img.Height }
    $len = $blobs[$i].Length
    $bw.Write([byte]$w)
    $bw.Write([byte]$h)
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([uint16]1)
    $bw.Write([uint16]32)
    $bw.Write([uint32]$len)
    $bw.Write([uint32]$offset)
    $offset += $len
  }
  for ($i = 0; $i -lt $count; $i++) {
    $bw.Write($blobs[$i])
  }
  $bw.Flush()
  [System.IO.File]::WriteAllBytes($path, $ms.ToArray())
  $bw.Dispose()
  $ms.Dispose()
}

$root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $root "src-tauri"))) {
  throw "Could not find src-tauri relative to $PSScriptRoot"
}
$iconDir = Join-Path $root "src-tauri\icons"
$publicDir = Join-Path $root "public"
$instRust = Join-Path $root "src-tauri\instance-icons"
$instPublic = Join-Path $root "public\instance-icons"
New-Item -ItemType Directory -Force -Path $iconDir, $instRust, $instPublic | Out-Null

$appTop = [System.Drawing.Color]::FromArgb(255, 45, 212, 132)
$appBot = [System.Drawing.Color]::FromArgb(255, 16, 140, 96)
$app256 = New-Mark 256 $appTop $appBot "WM"
$app128 = New-Mark 128 $appTop $appBot "WM"
$app32 = New-Mark 32 $appTop $appBot "WM"
$app48 = New-Mark 48 $appTop $appBot "WM"
$app256.Save((Join-Path $iconDir "icon-256.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$app128.Save((Join-Path $iconDir "128x128.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$app32.Save((Join-Path $iconDir "32x32.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$app256.Save((Join-Path $publicDir "launcher-avatar.png"), [System.Drawing.Imaging.ImageFormat]::Png)
Save-Ico @($app32, $app48) (Join-Path $iconDir "icon.ico")

$palette = @(
  @([System.Drawing.Color]::FromArgb(255,45,212,132), [System.Drawing.Color]::FromArgb(255,16,140,96), "A"),
  @([System.Drawing.Color]::FromArgb(255,56,189,248), [System.Drawing.Color]::FromArgb(255,14,116,144), "B"),
  @([System.Drawing.Color]::FromArgb(255,192,132,252), [System.Drawing.Color]::FromArgb(255,109,40,217), "C"),
  @([System.Drawing.Color]::FromArgb(255,251,146,60), [System.Drawing.Color]::FromArgb(255,194,65,12), "D"),
  @([System.Drawing.Color]::FromArgb(255,248,113,113), [System.Drawing.Color]::FromArgb(255,185,28,28), "E"),
  @([System.Drawing.Color]::FromArgb(255,250,204,21), [System.Drawing.Color]::FromArgb(255,202,138,4), "F"),
  @([System.Drawing.Color]::FromArgb(255,45,212,191), [System.Drawing.Color]::FromArgb(255,15,118,110), "G"),
  @([System.Drawing.Color]::FromArgb(255,244,114,182), [System.Drawing.Color]::FromArgb(255,157,23,77), "H")
)
for ($i = 0; $i -lt 8; $i++) {
  $item = $palette[$i]
  $bmp = New-Mark 256 $item[0] $item[1] $item[2]
  $name = "{0:D2}.png" -f ($i + 1)
  $bmp.Save((Join-Path $instRust $name), [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Save((Join-Path $instPublic $name), [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}

$app256.Dispose(); $app128.Dispose(); $app32.Dispose(); $app48.Dispose()
Write-Host "icons written"
Get-Item (Join-Path $iconDir "icon.ico") | Select-Object FullName, Length
