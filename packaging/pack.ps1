# Builds dist/WindowRescue.msix for the Microsoft Store from dist/WindowRescue.exe.
#
#   ./build.ps1
#   ./packaging/pack.ps1 -IdentityName <name> -Publisher <CN=...> -PublisherDisplayName <name>
#
# The three values come from Partner Center (Product management > Product identity); the
# defaults are placeholders, fine for a local test package. The package is left unsigned:
# the Store signs it. Needs MakeAppx and MakePri from the Windows SDK — the release workflow
# runs this on a GitHub runner, which has them.
param(
    [string]$IdentityName = 'okzea.WindowRescue',
    [string]$Publisher = 'CN=okzea',
    [string]$PublisherDisplayName = 'okzea'
)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot
$dist = Join-Path $root 'dist'
$exe = Join-Path $dist 'WindowRescue.exe'
if (-not (Test-Path $exe)) { throw 'dist\WindowRescue.exe is missing: run ./build.ps1 first.' }

$tool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*\x64\makeappx.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $tool) { throw 'MakeAppx not found: install the Windows SDK, or let the release workflow build the package.' }
$sdk = $tool.DirectoryName

$version = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$layout = Join-Path $dist 'msix'
Remove-Item -Recurse -Force $layout -ErrorAction SilentlyContinue
New-Item -ItemType Directory $layout | Out-Null
Copy-Item $exe $layout
Copy-Item (Join-Path $PSScriptRoot 'Assets') $layout -Recurse

$manifest = (Get-Content -Raw (Join-Path $PSScriptRoot 'AppxManifest.xml')).
    Replace('{IdentityName}', $IdentityName).
    Replace('{Publisher}', [Security.SecurityElement]::Escape($Publisher)).
    Replace('{PublisherDisplayName}', [Security.SecurityElement]::Escape($PublisherDisplayName)).
    Replace('{Version}', "$version.0")
[IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest)

# resources.pri lets Windows pick the right logo for each scale and size.
$priConfig = Join-Path $dist 'priconfig.xml'
& "$sdk\makepri.exe" createconfig /cf $priConfig /dq en-US /pv 10.0.0 /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed ($LASTEXITCODE)" }
& "$sdk\makepri.exe" new /pr $layout /cf $priConfig /mn (Join-Path $layout 'AppxManifest.xml') /of (Join-Path $layout 'resources.pri') /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri new failed ($LASTEXITCODE)" }

$msix = Join-Path $dist 'WindowRescue.msix'
& "$sdk\makeappx.exe" pack /d $layout /p $msix /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed ($LASTEXITCODE)" }
Write-Host "Packaged $msix ($IdentityName $version.0)"
