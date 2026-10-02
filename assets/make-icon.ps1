# Builds every icon from assets/logo.png (the master artwork, square, transparent background):
# assets/WindowRescue.ico for the exe and tray, and the Store logos in packaging/Assets.
#
#   ./assets/make-icon.ps1
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'

$logo = [System.Drawing.Image]::FromFile((Join-Path $PSScriptRoot 'logo.png'))

# The logo on a transparent canvas of $size, drawn at $fill of its width and centered.
function Draw([int]$size, [float]$fill = 1) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.CompositingQuality = 'HighQuality'
    $inner = $size * $fill
    $offset = ($size - $inner) / 2
    $g.DrawImage($logo, $offset, $offset, $inner, $inner)
    $g.Dispose()
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    , $ms.ToArray()
}

# .ico: every size stored as PNG.
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
    'Square150x150Logo.png'            = 150, 0.7
    'Square150x150Logo.scale-200.png'  = 300, 0.7
}
foreach ($t in 16, 24, 32, 48, 256) {
    $files["Square44x44Logo.targetsize-$t.png"] = $t, 1
    $files["Square44x44Logo.targetsize-${t}_altform-unplated.png"] = $t, 1
}
foreach ($name in $files.Keys) {
    $size, $fill = $files[$name]
    [System.IO.File]::WriteAllBytes((Join-Path $logos $name), (Draw $size $fill))
}
$logo.Dispose()
