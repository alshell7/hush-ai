# Build the Hush AI Microphone driver package for x64 and ARM64.
#   powershell -ExecutionPolicy Bypass -File virtual_mic\windows\build.ps1 [-Platforms x64,ARM64] [-Configuration Release]
# Needs Visual Studio 2022 with the Windows Driver Kit component and WDK 10.0.26100.
# Output: virtual_mic\windows\out\<platform>\ (HushMic.sys, HushMic.inf, hushmic.cat, HushMic.cer).
# The package is signed with the WDK test certificate. Loading it needs Windows test-signing
# mode; see README.md. Release packages need Microsoft attestation signing instead.
param(
    [string[]]$Platforms = @('x64', 'ARM64'),
    [string]$Configuration = 'Release'
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$kit = "${env:ProgramFiles(x86)}\Windows Kits\10"
$version = '10.0.26100.0'
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$msbuild = & $vswhere -latest -find 'MSBuild\**\Bin\MSBuild.exe' | Select-Object -First 1
if (-not $msbuild) { throw 'MSBuild not found' }
$signtool = "$kit\bin\$version\x64\signtool.exe"
$inf2cat = "$kit\bin\$version\x86\Inf2Cat.exe"
$infverif = "$kit\Tools\$version\x64\infverif.exe"

foreach ($platform in $Platforms) {
    # The WDK build tasks call x86 InfVerif and ApiValidator helpers that fail with this WDK;
    # the x64 tools run below instead.
    foreach ($project in 'EndpointsCommon\EndpointsCommon.vcxproj', 'TabletAudioSample\TabletAudioSample.vcxproj') {
        & $msbuild (Join-Path $here $project) /nologo /verbosity:minimal /m `
            /p:Configuration=$Configuration /p:Platform=$platform /p:SignMode=TestSign `
            /p:SkipPackageVerification=true /p:ApiValidator_Enable=false
        if ($LASTEXITCODE) { throw "Build failed: $project $platform" }
    }
    $built = Join-Path $here "TabletAudioSample\$platform\$Configuration"
    $out = Join-Path $here "out\$platform"
    Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $out | Out-Null
    Copy-Item "$built\HushMic.sys", "$built\HushMic.inf", "$built\HushMic.cer" $out

    $os = if ($platform -eq 'ARM64') { '10_GE_ARM64' } else { '10_GE_X64' }
    & $inf2cat /driver:$out /os:$os /uselocaltime | Out-Null
    if ($LASTEXITCODE) { throw "Inf2Cat failed for $platform" }
    $cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2 "$out\HushMic.cer"
    & $signtool sign /q /fd SHA256 /s My /sha1 $cert.Thumbprint "$out\hushmic.cat"
    if ($LASTEXITCODE) { throw "Signing the catalog failed for $platform" }
    $ddis = "$kit\build\$version\universalDDIs\$($platform.ToLower())"
    & "$kit\bin\$version\x64\apivalidator.exe" -DriverPackagePath:"$out\HushMic.sys" `
        -SupportedApiXmlFiles:"$ddis\UniversalDDIs.xml" -ModuleWhiteListXmlFiles:"$ddis\ModuleWhiteList.xml" `
        -ApiExtractorExePath:"$kit\bin\$version\x64"
    if ($LASTEXITCODE) { throw "ApiValidator failed for $platform" }
    & $infverif /v /w "$out\HushMic.inf"
    if ($LASTEXITCODE) { throw "InfVerif failed for $platform" }
    Get-ChildItem $out | Select-Object Name, Length | Format-Table | Out-String | Write-Host
}
