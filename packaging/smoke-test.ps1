# CI only: installs dist/WindowRescue.msix on the runner and checks it starts as a packaged app.
# Windows only installs signed packages, so a copy is signed with a throwaway certificate that
# is trusted on the runner alone. The Store package itself stays unsigned. Needs admin.
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot
$dist = Join-Path $root 'dist'
$manifest = [xml](Get-Content (Join-Path $dist 'msix\AppxManifest.xml'))
$publisher = $manifest.Package.Identity.Publisher
$name = $manifest.Package.Identity.Name

$sdk = (Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*\x64\signtool.exe" |
    Sort-Object FullName -Descending | Select-Object -First 1).DirectoryName

$test = Join-Path $dist 'WindowRescue-test.msix'
Copy-Item (Join-Path $dist 'WindowRescue.msix') $test -Force
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $publisher -CertStoreLocation Cert:\CurrentUser\My `
    -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
$pfx = Join-Path $dist 'test.pfx'
$password = ConvertTo-SecureString ([guid]::NewGuid().ToString()) -AsPlainText -Force
Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $password | Out-Null
Import-PfxCertificate -FilePath $pfx -CertStoreLocation Cert:\LocalMachine\TrustedPeople -Password $password | Out-Null
& "$sdk\signtool.exe" sign /fd SHA256 /f $pfx /p ([Net.NetworkCredential]::new('', $password).Password) $test
if ($LASTEXITCODE -ne 0) { throw "signtool failed ($LASTEXITCODE)" }

Add-AppxPackage $test
try {
    $package = Get-AppxPackage $name
    Write-Host "Installed $($package.PackageFullName)"
    Start-Process "shell:AppsFolder\$($package.PackageFamilyName)!WindowRescue"
    Start-Sleep 5
    $process = Get-Process WindowRescue -ErrorAction SilentlyContinue | Where-Object Path -like '*\WindowsApps\*'
    if (-not $process) { throw 'The packaged app did not start.' }
    Write-Host "Running packaged from $($process.Path)"
    $process | Stop-Process -Force
}
finally {
    Remove-AppxPackage $package.PackageFullName -ErrorAction SilentlyContinue
    Remove-Item $test, $pfx -ErrorAction SilentlyContinue
}
