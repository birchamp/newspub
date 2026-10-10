; Inno Setup script for the newpub Windows installer.
; Build: ISCC.exe /DAppVersion=0.1.0 /DSourceDir=<dir with newpub.exe and newpub.ico> /O<out dir> newpub.iss
; Installs per user (no administrator prompt), adds a Start menu entry, an optional desktop icon,
; the .npub file association and an uninstaller.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef SourceDir
  #define SourceDir "."
#endif

[Setup]
AppId={{6E3F2A41-8B7C-4D2E-9A51-3C0B7E9D4F12}
AppName=newpub
AppVersion={#AppVersion}
AppVerName=newpub {#AppVersion}
AppPublisher=newpub contributors
DefaultDirName={autopf}\newpub
DefaultGroupName=newpub
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=newpub-{#AppVersion}-windows-x64-setup
SetupIconFile={#SourceDir}\newpub.ico
UninstallDisplayIcon={app}\newpub.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\newpub.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\newpub-agent.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\newpub.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\newpub"; Filename: "{app}\newpub.exe"; IconFilename: "{app}\newpub.ico"
Name: "{autodesktop}\newpub"; Filename: "{app}\newpub.exe"; IconFilename: "{app}\newpub.ico"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.npub"; ValueType: string; ValueName: ""; ValueData: "newpub.Publication"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\newpub.Publication"; ValueType: string; ValueName: ""; ValueData: "newpub publication"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\newpub.Publication\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\newpub.ico"
Root: HKA; Subkey: "Software\Classes\newpub.Publication\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\newpub.exe"" ""%1"""
; Publisher files list newpub under "Open with" without changing their default app.
Root: HKA; Subkey: "Software\Classes\.pub\OpenWithProgids"; ValueType: string; ValueName: "newpub.Publication"; ValueData: ""; Flags: uninsdeletevalue

[Run]
Filename: "{app}\newpub.exe"; Description: "{cm:LaunchProgram,newpub}"; Flags: nowait postinstall skipifsilent
