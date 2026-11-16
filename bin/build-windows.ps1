# Build volta for 64-bit Windows on a Windows machine.
#
# Needs the MSVC build tools (Visual Studio Build Tools with the C++ workload),
# Rust from rustup, and nothing else. The result is copied to
# target\windows as volta.exe and packaged into a zip.
$ErrorActionPreference = "Stop"

Set-Location (Split-Path $PSScriptRoot -Parent)

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust from https://rustup.rs first."
}

cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$out = "target\windows"
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item "target\release\volta.exe" "$out\volta.exe" -Force
if (Test-Path "docs\windows.md") {
    Copy-Item "docs\windows.md" "$out\README-windows.md" -Force
}

$zip = Join-Path $out "volta-windows-x86_64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
$files = @("$out\volta.exe")
if (Test-Path "$out\README-windows.md") { $files += "$out\README-windows.md" }
Compress-Archive -Path $files -DestinationPath $zip -CompressionLevel Optimal

Write-Host "Windows build: $out\volta.exe"
Write-Host "Package:       $zip"
