<#
.SYNOPSIS
    Prépare la mise à jour automatique de Calame. À lancer une seule fois, par la personne qui publie.

.DESCRIPTION
    1. Crée la paire de clés de signature avec l'outil de Tauri, hors du dépôt
       (%USERPROFILE%\.calame\updater.key). Une clé existante est réutilisée, jamais écrasée.
    2. Range la clé privée dans les secrets du dépôt GitHub, pour la publication.
    3. Écrit la clé publique dans crates\calame-app\updater.pub, à enregistrer dans git.

    Il faut Node.js (npx) et GitHub CLI (gh), connecté à un compte qui administre le dépôt.

    Gardez une copie de la clé et de son mot de passe : sans eux, les versions déjà installées
    refuseront les mises à jour suivantes.
#>
$ErrorActionPreference = 'Stop'
$repo = 'Teamfliyou/calame'
$cli = '@tauri-apps/cli@2.12.6'
$root = Split-Path -Parent $PSScriptRoot
$keyDir = Join-Path $env:USERPROFILE '.calame'
$key = Join-Path $keyDir 'updater.key'

foreach ($tool in 'npx', 'gh') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "$tool est introuvable" }
}

if (Test-Path $key) {
    Write-Host "Clé existante réutilisée : $key"
} else {
    New-Item -ItemType Directory -Force -Path $keyDir | Out-Null
    Write-Host "Création de la clé. Choisissez un mot de passe (ou laissez vide) et notez-le."
    npx --yes $cli signer generate -w $key
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $key)) { throw "La clé n'a pas été créée" }
}

# Redirection par cmd : le fichier est transmis tel quel, sans passer par la ligne de commande.
cmd /c "gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $repo < `"$key`""
if ($LASTEXITCODE -ne 0) { throw "Le secret TAURI_SIGNING_PRIVATE_KEY n'a pas été enregistré" }

$answer = Read-Host "La clé a-t-elle un mot de passe ? (o/N)"
if ($answer -match '^[oOyY]') {
    Write-Host "Saisissez ce mot de passe pour le secret GitHub :"
    gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo $repo
    if ($LASTEXITCODE -ne 0) { throw "Le mot de passe n'a pas été enregistré" }
}

Copy-Item "$key.pub" (Join-Path $root 'crates\calame-app\updater.pub') -Force
Write-Host ""
Write-Host "C'est prêt. Enregistrez crates\calame-app\updater.pub dans git, puis publiez la version."
