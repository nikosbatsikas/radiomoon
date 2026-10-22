# Builds the portable RadioMoon packages for Windows: x64, x86 and arm64 (one zip each).
# Usage (PowerShell, in this folder):
#   .\build_portable.ps1                                   # all three editions
#   .\build_portable.ps1 -Arch x64 -MariaDbZip C:\mariadb-11.4.4-winx64.zip
# Needs Rust (MSVC toolchain) and Visual Studio Build Tools with the x86 and ARM64 components.
# The bundled MariaDB is only available for x64; x86/arm64 editions use an external MySQL/MariaDB.
param([string[]]$Arch = @("x64", "x86", "arm64"), [string]$MariaDbZip = "")
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$targets = @{ x64 = "x86_64-pc-windows-msvc"; x86 = "i686-pc-windows-msvc"; arm64 = "aarch64-pc-windows-msvc" }
$v = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
New-Item -ItemType Directory -Force -Path dist | Out-Null

foreach ($a in $Arch) {
    $t = $targets[$a]
    if (-not $t) { throw "Unknown architecture '$a' (use x64, x86, arm64)" }
    Write-Host "=== $a ($t) ==="
    rustup target add $t
    cargo build --release --target $t

    $dist = Join-Path $PSScriptRoot "dist\RadioMoon-$v-windows-$a-portable"
    if (Test-Path $dist) { Remove-Item $dist -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    Copy-Item "target\$t\release\radiomoon.exe" "$dist\RadioMoon.exe" -Force
    Copy-Item "target\$t\release\radiomoon-mpx-receiver.exe" "$dist\RadioMoon-MPX-Receiver.exe" -Force
    foreach ($d in "music", "jingles", "commercials", "lang") { New-Item -ItemType Directory -Force -Path "$dist\$d" | Out-Null }
    Copy-Item "README.md", "CHANGELOG.md" $dist -Force
    Copy-Item "lang\*" "$dist\lang" -Force
    New-Item -ItemType File -Force -Path "$dist\portable.flag" | Out-Null

    if ($a -eq "x64" -and $MariaDbZip -ne "") {
        $tmp = Join-Path $env:TEMP "radiomoon_mariadb"
        if (Test-Path $tmp) { Remove-Item $tmp -Recurse -Force }
        Expand-Archive -Path $MariaDbZip -DestinationPath $tmp
        $root = Get-ChildItem $tmp | Where-Object { $_.PSIsContainer } | Select-Object -First 1
        Move-Item $root.FullName (Join-Path $dist "mariadb")
        Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }

    $zip = "dist\RadioMoon-$v-windows-$a-portable.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path "$dist\*" -DestinationPath $zip
    Write-Host "Zip: $zip"
}
