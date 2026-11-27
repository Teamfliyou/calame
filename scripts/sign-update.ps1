<#
.SYNOPSIS
    Signe l'installeur et écrit dist\latest.json, que Calame lit pour se mettre à jour.

.DESCRIPTION
    Appelé par le workflow de publication, après package-windows.ps1. La clé privée vient de la
    variable TAURI_SIGNING_PRIVATE_KEY (et TAURI_SIGNING_PRIVATE_KEY_PASSWORD).

    Sans clé publique dans crates\calame-app\updater.pub, la mise à jour n'est pas configurée :
    rien n'est signé. Avec une clé publique mais sans clé privée, la publication échoue.
#>
$ErrorActionPreference = 'Stop'
$repo = 'Teamfliyou/calame'
$cli = '@tauri-apps/cli@2.12.6'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$public = Join-Path $root 'crates\calame-app\updater.pub'
if (-not (Test-Path $public) -or -not "$(Get-Content $public -Raw)".Trim()) {
    Write-Host "::warning::Mise à jour automatique non configurée (scripts\setup-updater.ps1) : pas de latest.json."
    exit 0
}
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    throw "Le secret TAURI_SIGNING_PRIVATE_KEY est absent : lancez scripts\setup-updater.ps1"
}

$manifest = Get-Content (Join-Path $root 'Cargo.toml') -Raw
if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw "Version introuvable dans Cargo.toml" }
$version = $Matches[1]
$setup = "calame-$version-setup.exe"
$path = Join-Path $root "dist\$setup"
if (-not (Test-Path $path)) { throw "Installeur introuvable : $path" }

$ErrorActionPreference = 'Continue'
npx --yes $cli signer sign $path
$signExit = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
if ($signExit -ne 0) { throw "La signature a échoué" }

$latest = [ordered]@{
    version   = $version
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = (Get-Content "$path.sig" -Raw).Trim()
            url       = "https://github.com/$repo/releases/download/v$version/$setup"
            # « nsis » désigne pour l'outil de mise à jour un installeur .exe, ici Inno Setup.
            format    = 'nsis'
        }
    }
}
# Sans marque d'ordre des octets : le lecteur JSON la refuserait.
$json = $latest | ConvertTo-Json -Depth 5
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText((Join-Path $root 'dist\latest.json'), $json, $utf8)
Write-Host "Description de la mise à jour : dist\latest.json"
