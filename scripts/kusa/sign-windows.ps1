# Authenticode-sign the Windows runtime binaries in a bundle directory.
#
# The interface is in place; the certificate is not (kusa-desktop DESIGN Q6).
# Until WINDOWS_SIGNING_PFX is set this only says so and leaves the files
# unsigned. Once it is:
#   WINDOWS_SIGNING_PFX       base64 of the code-signing PFX (repository secret)
#   WINDOWS_SIGNING_PASSWORD  its password (repository secret)
#   WINDOWS_TIMESTAMP_URL     RFC 3161 timestamp server (repository variable)
# Every PE file in the bundle is signed (SHA-256, timestamped) and verified.
# libkrunfw.dll is an upstream binary we do not rebuild; we sign it after the
# provenance check, as DESIGN 10.5 says.
param([Parameter(Mandatory = $true)][string]$Dir)
$ErrorActionPreference = 'Stop'

$files = Get-ChildItem -Path $Dir -File | Where-Object { $_.Extension -in '.dll', '.exe' }
if (-not $env:WINDOWS_SIGNING_PFX) {
    Write-Output "::notice::No signing certificate configured; the bundle is unsigned."
    $files | ForEach-Object { Write-Output "  unsigned: $($_.Name)" }
    exit 0
}

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found" }
if (-not $env:WINDOWS_TIMESTAMP_URL) { throw "WINDOWS_TIMESTAMP_URL is not set" }

$pfx = Join-Path $env:RUNNER_TEMP 'signing.pfx'
[IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_SIGNING_PFX))
try {
    foreach ($f in $files) {
        & $signtool.FullName sign /fd SHA256 /f $pfx /p $env:WINDOWS_SIGNING_PASSWORD `
            /tr $env:WINDOWS_TIMESTAMP_URL /td SHA256 $f.FullName
        if ($LASTEXITCODE -ne 0) { throw "signing $($f.Name) failed" }
        & $signtool.FullName verify /pa $f.FullName
        if ($LASTEXITCODE -ne 0) { throw "verifying $($f.Name) failed" }
    }
}
finally {
    Remove-Item $pfx -Force -ErrorAction SilentlyContinue
}
