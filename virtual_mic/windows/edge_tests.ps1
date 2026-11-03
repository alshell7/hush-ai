# Hush AI Microphone edge cases on Windows, after install.ps1. Run from the repository root:
#   cargo build --release -p hush-core --features devices --example virtual_mic_check
#   powershell -ExecutionPolicy Bypass -File virtual_mic\windows\edge_tests.ps1 [-RestartAudioService]
# -RestartAudioService also restarts the Windows audio service mid-test (administrator).
# Each check prints one JSON line; a failed check exits non-zero. Same cases as macOS.
param([switch]$RestartAudioService)
$check = '.\target\release\examples\virtual_mic_check.exe'
$failed = 0
function Run($name, [string[]]$arguments, [switch]$ExpectFailure) {
    $output = & $check @arguments 2>&1 | Out-String
    $ok = if ($ExpectFailure) { $LASTEXITCODE -ne 0 } else { $LASTEXITCODE -eq 0 }
    Write-Host ("{0,-20} {1} {2}" -f $name, ($(if ($ok) { 'PASS' } else { 'FAIL' })), $output.Trim())
    if (-not $ok) { $script:failed++ }
}
Run 'baseline' '--seconds', '20'
Run 'buffer-64' '--seconds', '15', '--buffer', '64'
Run 'buffer-4096' '--seconds', '15', '--buffer', '4096'
Run 'writer-stops' '--seconds', '10', '--stop-writer-after', '5'
Run 'writer-starts-late' '--seconds', '10', '--start-writer-after', '4'
Run 'fill-ring' '--write-only', '--seconds', '3'
Run 'stale-ring' '--record-only', '--seconds', '5'

# Two recording apps while one writes; a second writer must be refused.
$writer = Start-Process $check -ArgumentList '--write-only', '--seconds', '22' -PassThru -NoNewWindow
Start-Sleep 2
$client2 = Start-Process $check -ArgumentList '--record-only', '--expect-tone', '--seconds', '15' -PassThru -NoNewWindow
Run 'client1' '--record-only', '--expect-tone', '--seconds', '15'
$client2.WaitForExit(); if ($client2.ExitCode) { Write-Host 'client2 FAIL'; $failed++ } else { Write-Host 'client2 PASS' }
Run 'second-writer' '--write-only', '--seconds', '2' -ExpectFailure
$writer.WaitForExit(); if ($writer.ExitCode) { Write-Host 'writer FAIL'; $failed++ }

if ($RestartAudioService) {
    $writer = Start-Process $check -ArgumentList '--write-only', '--seconds', '25' -PassThru -NoNewWindow
    Start-Sleep 3
    Restart-Service audiosrv -Force
    Start-Sleep 5
    Run 'after-restart' '--record-only', '--expect-tone', '--seconds', '8'
    $writer.WaitForExit()
}

# Full CPU load: one busy process per logical processor.
$load = 1..[Environment]::ProcessorCount | ForEach-Object {
    Start-Process powershell -ArgumentList '-NoProfile', '-Command', 'while ($true) {}' -WindowStyle Hidden -PassThru
}
try { Run 'cpu-load' '--seconds', '20' } finally { $load | Stop-Process -Force -ErrorAction SilentlyContinue }

if ($failed) { Write-Host "$failed check(s) failed"; exit 1 }
Write-Host 'All edge cases passed.'
