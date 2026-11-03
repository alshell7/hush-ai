# Install the test-signed Hush AI Microphone driver on this computer.
# Run from an administrator PowerShell, after build.ps1:
#   powershell -ExecutionPolicy Bypass -File virtual_mic\windows\install.ps1 [-Uninstall]
#
# Test-signed drivers load only in Windows test-signing mode. This script does not change
# that setting. To turn it on (once, then restart):
#   bcdedit /set testsigning on
# Secure Boot must be off for test signing. Turn it off in the firmware settings if needed.
# Turn test signing off again with "bcdedit /set testsigning off" when you stop testing.
param([switch]$Uninstall)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'ARM64' } else { 'x64' }
$package = Join-Path $here "out\$arch"
$kitTools = "${env:ProgramFiles(x86)}\Windows Kits\10\Tools\10.0.26100.0\$($arch.ToLower())"
$devcon = Join-Path $kitTools 'devcon.exe'
$hardwareId = 'Root\HushAIMicrophone'

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { throw 'Run this script from an administrator PowerShell.' }
if (-not (Test-Path $devcon)) { throw "devcon.exe not found in $kitTools (install the WDK)." }

if ($Uninstall) {
    & $devcon remove $hardwareId
    # Remove the driver package from the driver store.
    pnputil /enum-drivers | Select-String -Context 0, 6 'hushmic.inf' | ForEach-Object {
        $published = ($_.Line -split ':\s*')[1]
        if ($published) { pnputil /delete-driver $published /uninstall /force }
    }
    Write-Host 'Hush AI Microphone removed.'
    return
}

if (-not (Test-Path "$package\HushMic.inf")) { throw "No package in $package. Run build.ps1 first." }
$testSigning = (bcdedit /enum '{current}') -match 'testsigning\s+Yes'
if (-not $testSigning) {
    throw 'Windows test-signing mode is off. Run "bcdedit /set testsigning on", restart, then run this again.'
}
# Trust the WDK test certificate on this computer only (test signing).
Import-Certificate -FilePath "$package\HushMic.cer" -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
Import-Certificate -FilePath "$package\HushMic.cer" -CertStoreLocation Cert:\LocalMachine\TrustedPublisher | Out-Null
& $devcon install "$package\HushMic.inf" $hardwareId
if ($LASTEXITCODE -gt 1) { throw "devcon install failed with $LASTEXITCODE" } # 1 = restart needed
Write-Host 'Installed. "Hush AI Microphone" should now appear under Settings > System > Sound > Input.'
