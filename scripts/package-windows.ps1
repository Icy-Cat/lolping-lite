# Build a portable Windows zip for LolPing Lite.
# Does NOT bundle assets — CDN on-demand.
# Usage (from repo root, after cargo build --release):
#   powershell -File scripts/package-windows.ps1
# Optional: -Binary path\to\lolping-lite.exe  -OutDir dist\windows
param(
  [string]$Binary = "",
  [string]$OutDir = "dist\windows"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$Version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value
if (-not $Binary) {
  $Binary = Join-Path $Root "target\release\lolping-lite.exe"
}
if (-not (Test-Path $Binary)) {
  throw "missing binary: $Binary — run cargo build --release first"
}

$Stage = Join-Path $OutDir "LolPing-Lite-$Version-windows-x64"
$Zip = Join-Path $OutDir "LolPing-Lite-$Version-windows-x64.zip"
Remove-Item -Recurse -Force $OutDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item $Binary (Join-Path $Stage "lolping-lite.exe")
Copy-Item LICENSE (Join-Path $Stage "LICENSE")
Copy-Item ASSETS.md (Join-Path $Stage "ASSETS.md")
Copy-Item README.md (Join-Path $Stage "README.md")

@"
LolPing Lite $Version (Windows x64 portable)

Run lolping-lite.exe. First launch downloads emotes/sounds on demand from
https://lolping-assets.icy-cat.com/ (or open Settings → Assets → Download all).

VP9 / v2 WebM: only if this build was linked with libvpx; otherwise v1 WebP.

Accessibility / global hooks need the app elevated or permitted as needed on
your Windows version. Not endorsed by Riot Games. Code MIT; assets © Riot.
"@ | Set-Content -Encoding UTF8 (Join-Path $Stage "README-WINDOWS.txt")

if (Test-Path $Zip) { Remove-Item $Zip }
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip -Force

Write-Host "Stage: $Stage"
Write-Host "Zip:   $Zip"
Get-Item $Zip | Format-List FullName, Length
