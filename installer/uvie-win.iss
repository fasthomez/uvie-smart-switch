; uvie-win.iss — Inno Setup script for UVie for Windows.
;
; Bundles:
;   - uvie-win.exe                              (the IME, built by cargo --release)
;   - Microsoft.WindowsAppRuntime.Bootstrap.dll (unpackaged WASDK loader stub)
;   - WindowsAppRuntimeInstall-x64.exe          (Microsoft redist, fetched into
;                                              Frameworks/ by scripts/fetch-windowsappsdk)
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

[Files]
Source: "..\target\release\uvie-win.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\Microsoft.WindowsAppRuntime.Bootstrap.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Frameworks\WindowsAppSDK\WindowsAppRuntimeInstall-x64.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#ExeName}"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#ExeName}"; Tasks: desktopicon

[Run]
; The WinUI 3 settings window needs the Windows App Runtime (framework +
; DDLM + main packages). The redist is idempotent — a fast no-op when a
; compatible runtime is already present — so always run it. Must complete
; before the app is offered for launch.
Filename: "{tmp}\WindowsAppRuntimeInstall-x64.exe"; Parameters: "--quiet"; StatusMsg: "Installing Windows App Runtime 1.6..."; Flags: waituntilterminated
Filename: "{app}\{#ExeName}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent unchecked
