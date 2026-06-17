param(
    [string]$Version
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if (-not $Version) {
    $CargoToml = Get-Content "Cargo.toml" -Raw
    if ($CargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        Write-Error "Could not read version from Cargo.toml"
    }
}

cargo build --release
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$Iscc = Get-Command iscc -ErrorAction SilentlyContinue
if (-not $Iscc) {
    $Candidate = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    if (Test-Path $Candidate) {
        $IsccPath = $Candidate
    } else {
        Write-Error "Inno Setup not found. Install from https://jrsoftware.org/isinfo.php"
    }
} else {
    $IsccPath = $Iscc.Source
}

& $IsccPath "/DMyAppVersion=$Version" "installer\winharpoon.iss"
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "Installer written to dist\WinHarpoon-Setup-$Version.exe"
