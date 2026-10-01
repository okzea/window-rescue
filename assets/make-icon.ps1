# Draws assets/WindowRescue.ico: a window with an arrow bringing it back, on a blue tile.
# Every size is drawn from the same 256-unit layout and stored as PNG inside the .ico.
#
#   ./assets/make-icon.ps1
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'

function Rounded([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $p.AddArc($x, $y, 2 * $r, 2 * $r, 180, 90)
    $p.AddArc($x + $w - 2 * $r, $y, 2 * $r, 2 * $r, 270, 90)
    $p.AddArc($x + $w - 2 * $r, $y + $h - 2 * $r, 2 * $r, 2 * $r, 0, 90)
    $p.AddArc($x, $y + $h - 2 * $r, 2 * $r, 2 * $r, 90, 90)
    $p.CloseFigure()
    $p
}

function Draw([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.ScaleTransform($size / 256, $size / 256)

    $tile = Rounded 8 8 240 240 52
    $top = [System.Drawing.Color]::FromArgb(59, 130, 246)
    $bottom = [System.Drawing.Color]::FromArgb(29, 78, 216)
    $fill = New-Object System.Drawing.Drawing2D.LinearGradientBrush ([System.Drawing.PointF]::new(0, 8)), ([System.Drawing.PointF]::new(0, 248)), $top, $bottom
    $g.FillPath($fill, $tile)

    # The window: white body, light-blue title bar.
    $window = Rounded 36 68 132 120 16
    $g.FillPath([System.Drawing.Brushes]::White, $window)
    $g.SetClip($window)
    $bar = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(147, 197, 253))
    $g.FillRectangle($bar, 36, 68, 132, 30)
    $g.ResetClip()

    # The arrow pointing back at it.
    $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), 22
    $pen.StartCap = 'Round'
    $pen.EndCap = 'Round'
    $pen.LineJoin = 'Round'
    $g.DrawLine($pen, 226, 128, 192, 128)
    $g.DrawLines($pen, [System.Drawing.PointF[]]@([System.Drawing.PointF]::new(210, 104), [System.Drawing.PointF]::new(186, 128), [System.Drawing.PointF]::new(210, 152)))

    $g.Dispose()
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    , $ms.ToArray()
}

$sizes = 16, 20, 24, 32, 40, 48, 64, 256
$images = foreach ($s in $sizes) { , (Draw $s) }

$out = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $out
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $dim = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
    $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$images[$i].Length); $w.Write([uint32]$offset)
    $offset += $images[$i].Length
}
foreach ($img in $images) { $w.Write($img) }
[System.IO.File]::WriteAllBytes((Join-Path $PSScriptRoot 'WindowRescue.ico'), $out.ToArray())
