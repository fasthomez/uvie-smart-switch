; uvie-win.iss — Inno Setup script for UVie for Windows.
;
; Bundles uvie-win.exe only — the settings UI is Slint now, so no Windows
; App SDK runtime or bootstrap DLL is needed.
;
; Per-user install into %LOCALAPPDATA%\Programs\UVie — no UAC, matching the
; app's own convention (settings live in %APPDATA%\UVie, launch-at-login is a
; per-user HKCU Run value the app manages itself).
;
; Build:
;   iscc installer\uvie-win.iss
;   iscc /DAppVersion=1.0.1 installer\uvie-win.iss   (CI passes the crate version)

#ifndef AppVersion
  #define AppVersion "1.0.1"
#endif

#define AppName "UVie for Windows"
#define AppPublisher "uvie-project"
#define ExeName "uvie-win.exe"

[Setup]
AppId={{41FA6A26-A298-4AB8-8C4D-B3FA575AC990}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={localappdata}\Programs\UVie
DefaultGroupName={#AppName}
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=uvie-win-{#AppVersion}-x64-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#ExeName}
; Kill the tray app before replacing its files on upgrade/reinstall.
CloseApplications=yes
CloseApplicationsFilter=uvie-win.exe
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional icons:"; Flags: unchecked

[Registry]
; Remove the app's own launch-at-login value if the user enabled it —
; otherwise uninstall leaves a dead HKCU Run entry pointing at a deleted exe.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "UVie"; Flags: uninsdeletevalue

[Files]
Source: "..\target\release\uvie-win.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#ExeName}"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#ExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#ExeName}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent unchecked
