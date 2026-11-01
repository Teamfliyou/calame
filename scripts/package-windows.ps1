<#
.SYNOPSIS
    Construit l'archive portable Windows de Calame : dist\calame-<version>-windows-x64.zip

.DESCRIPTION
    Compile le profil "dist" (optimisé, symboles retirés), rassemble l'exécutable, les licences et le
    README dans un dossier, puis le compresse. Si Inno Setup (iscc.exe) est installé, produit aussi
    l'installeur dist\calame-<version>-setup.exe.

.PARAMETER SkipBuild
    N'exécute pas cargo : réutilise target\dist\calame.exe déjà compilé.

.EXAMPLE
    .\scripts\package-windows.ps1
#>
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# La version vient du workspace : une seule source de vérité.
$manifest = Get-Content (Join-Path $root 'Cargo.toml') -Raw
if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    throw "Version introuvable dans Cargo.toml"
}
$version = $Matches[1]

if (-not $SkipBuild) {
    # cargo écrit sa progression sur la sortie d'erreur : sous Windows PowerShell 5.1, avec
    # 'Stop', cela passerait pour un échec. On ne se fie qu'au code de sortie.
    $ErrorActionPreference = 'Continue'
    cargo build --profile dist -p calame-app
    $buildExit = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($buildExit -ne 0) { throw "La compilation a échoué" }
}

$exe = Join-Path $root 'target\dist\calame.exe'
if (-not (Test-Path $exe)) { throw "Exécutable introuvable : $exe" }

$name = "calame-$version-windows-x64"
$stage = Join-Path $root "dist\$name"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage | Out-Null

Copy-Item $exe (Join-Path $stage 'Calame.exe')
foreach ($file in 'LICENSE-MIT', 'LICENSE-APACHE', 'README.md') {
    Copy-Item (Join-Path $root $file) $stage
}
Copy-Item (Join-Path $root 'docs\guide-utilisateur.md') $stage

$zip = Join-Path $root "dist\$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "Archive portable : $zip"

$iscc = Get-Command iscc.exe -ErrorAction SilentlyContinue
if ($iscc) {
    $ErrorActionPreference = 'Continue'
    & $iscc.Source "/DAppVersion=$version" (Join-Path $root 'packaging\windows\calame.iss')
    $isccExit = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($isccExit -ne 0) { throw "Inno Setup a échoué" }
    Write-Host "Installeur : dist\calame-$version-setup.exe"
} else {
    Write-Host "Inno Setup (iscc.exe) absent : pas d'installeur. Archive portable seulement."
}
