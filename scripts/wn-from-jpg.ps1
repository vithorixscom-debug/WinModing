$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

function Get-Dib32([System.Drawing.Bitmap]$bmp) {
  $w = $bmp.Width
  $h = $bmp.Height
  $xorStride = $w * 4
  $andStride = [int][Math]::Ceiling($w / 32.0) * 4
  $header = New-Object byte[] 40
  [BitConverter]::GetBytes([int]40).CopyTo($header, 0)
  [BitConverter]::GetBytes([int]$w).CopyTo($header, 4)
  [BitConverter]::GetBytes([int]($h * 2)).CopyTo($header, 8)
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
      if ($c.A -lt 128) {
        $andRow = ($h - 1 - $y) * $andStride
        $byteIndex = $andRow + [int][Math]::Floor($x / 8)
        $bit = 7 - ($x % 8)
        $and[$byteIndex] = $and[$byteIndex] -bor [byte](1 -shl $bit)
      }
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
  foreach ($img in $images) { $blobs.Add([byte[]](Get-Dib32 $img)) }
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
  for ($i = 0; $i -lt $count; $i++) { $bw.Write($blobs[$i]) }
  $bw.Flush()
  [System.IO.File]::WriteAllBytes($path, $ms.ToArray())
  $bw.Dispose()
  $ms.Dispose()
}

function Test-Lime([System.Drawing.Color]$c) {
  return ($c.G -gt 140 -and $c.R -gt 40 -and $c.B -lt 200 -and ($c.G - $c.B) -gt 40)
}

function Fill-SquareBleed([System.Drawing.Bitmap]$src) {
  $size = [Math]::Max($src.Width, $src.Height)
  $dst = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($dst)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)
  $r = [Math]::Max(8, [int]($size * 0.18))
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = $r * 2
  $path.AddArc(0, 0, $d, $d, 180, 90)
  $path.AddArc($size - $d, 0, $d, $d, 270, 90)
  $path.AddArc($size - $d, $size - $d, $d, $d, 0, 90)
  $path.AddArc(0, $size - $d, $d, $d, 90, 90)
  $path.CloseFigure()
  $g.SetClip($path)
  $g.DrawImage($src, 0, 0, $size, $size)
  $path.Dispose()
  $g.Dispose()
  return $dst
}

function Resize-Bmp([System.Drawing.Bitmap]$src, [int]$size) {
  $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
  $g.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
  $g.Clear([System.Drawing.Color]::Transparent)
  $g.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceOver
  $g.DrawImage($src, 0, 0, $size, $size)
  $g.Dispose()
  return $bmp
}

$root = Split-Path -Parent $PSScriptRoot
$iconDir = Join-Path $root "src-tauri\icons"
$srcPath = Join-Path $iconDir "wn-src.jpg"
if (-not (Test-Path $srcPath)) { throw "missing $srcPath" }

$src = [System.Drawing.Bitmap]::FromFile($srcPath)
$square = Fill-SquareBleed $src
$src.Dispose()

$png256 = Resize-Bmp $square 256
$png128 = Resize-Bmp $square 128
$png48 = Resize-Bmp $square 48
$png32 = Resize-Bmp $square 32
$png16 = Resize-Bmp $square 16
$square.Dispose()

New-Item -ItemType Directory -Force -Path $iconDir | Out-Null
$png256.Save((Join-Path $iconDir "icon-256.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$png128.Save((Join-Path $iconDir "128x128.png"), [System.Drawing.Imaging.ImageFormat]::Png)
$png32.Save((Join-Path $iconDir "32x32.png"), [System.Drawing.Imaging.ImageFormat]::Png)
Save-Ico @($png16, $png32, $png48, $png256) (Join-Path $iconDir "icon.ico")

$png256.Dispose(); $png128.Dispose(); $png48.Dispose(); $png32.Dispose(); $png16.Dispose()
Write-Host "wrote icons"
Get-Item (Join-Path $iconDir "icon.ico") | Format-List FullName, Length
