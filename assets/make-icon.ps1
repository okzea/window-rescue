# Draws the app icon 'a window with an arrow bringing it back, on a blue tile' into
# assets/WindowRescue.ico and the Store logos in packaging/Assets. Every image comes from the
# same 256-unit layout; the .ico stores each size as PNG.
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

# The icon on a transparent canvas of $size, drawn at $fill of its width and centered.
function Draw([int]$size, [float]$fill = 1) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.TranslateTransform($size * (1 - $fill) / 2, $size * (1 - $fill) / 2)
    $g.ScaleTransform($size * $fill / 256, $size * $fill / 256)

    $tile = Rounded 8 8 240 240 52
    $top = [System.Drawing.Color]::FromArgb(59, 130, 246)
    $bottom = [System.Drawing.Color]::FromArgb(29, 78, 216)
    $gradient = New-Object System.Drawing.Drawing2D.LinearGradientBrush ([System.Drawing.PointF]::new(0, 8)), ([System.Drawing.PointF]::new(0, 248)), $top, $bottom
    $g.FillPath($gradient, $tile)

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

# Store / MSIX logos. Unqualified files are the 100% scale; makepri picks the others by name.
$logos = Join-Path $PSScriptRoot '..\packaging\Assets'
New-Item -ItemType Directory -Force $logos | Out-Null
$files = [ordered]@{
    'StoreLogo.png'                    = 50, 1
    'StoreLogo.scale-200.png'          = 100, 1
    'Square44x44Logo.png'              = 44, 1
    'Square44x44Logo.scale-200.png'    = 88, 1
    'Square150x150Logo.png'            = 150, 0.6
    'Square150x150Logo.scale-200.png'  = 300, 0.6
}
foreach ($t in 16, 24, 32, 48, 256) {
    $files["Square44x44Logo.targetsize-$t.png"] = $t, 1
    $files["Square44x44Logo.targetsize-${t}_altform-unplated.png"] = $t, 1
}
foreach ($name in $files.Keys) {
    $size, $fill = $files[$name]
    [System.IO.File]::WriteAllBytes((Join-Path $logos $name), (Draw $size $fill))
}
