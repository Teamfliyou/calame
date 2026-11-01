; Installeur Windows de Calame, pour Inno Setup 6 (https://jrsoftware.org/isinfo.php).
; Construit par scripts\package-windows.ps1 ou par le workflow de publication :
;   iscc /DAppVersion=1.0.0 packaging\windows\calame.iss
; Prérequis : target\dist\calame.exe compilé (cargo build --profile dist -p calame-app).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6E2F1B7A-4C0D-4E58-9B7A-CA1A3E0C0001}
AppName=Calame
AppVersion={#AppVersion}
AppPublisher=Contributeurs de Calame
AppPublisherURL=https://github.com/Teamfliyou/calame
AppSupportURL=https://github.com/Teamfliyou/calame/issues
DefaultDirName={autopf}\Calame
DefaultGroupName=Calame
UninstallDisplayIcon={app}\Calame.exe
OutputDir=..\..\dist
OutputBaseFilename=calame-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
LicenseFile=..\..\LICENSE-MIT

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "Créer une icône sur le Bureau"; GroupDescription: "Icônes :"; Flags: unchecked

[Files]
Source: "..\..\target\dist\calame.exe"; DestDir: "{app}"; DestName: "Calame.exe"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\docs\guide-utilisateur.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Calame"; Filename: "{app}\Calame.exe"
Name: "{group}\Désinstaller Calame"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Calame"; Filename: "{app}\Calame.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Calame.exe"; Description: "Lancer Calame"; Flags: nowait postinstall skipifsilent

; Les notes (%APPDATA%\calame) ne sont volontairement pas supprimées à la désinstallation.
