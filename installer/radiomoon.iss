; Inno Setup script. Called by the release workflow:
;   ISCC /DAppVersion=0.2.0 /DArch=x64 /DSourceDir=...\stage\RadioMoon /DOutDir=...\dist installer\radiomoon.iss
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef SourceDir
  #define SourceDir "..\stage\RadioMoon"
#endif
#ifndef OutDir
  #define OutDir "..\dist"
#endif

[Setup]
AppId={{B6E1C6A2-5D3A-4F0B-9C55-4D6F0C8A1E21}
AppName=RadioMoon
AppVersion={#AppVersion}
AppPublisher=RadioMoon
DefaultDirName={autopf}\RadioMoon
DefaultGroupName=RadioMoon
PrivilegesRequired=admin
OutputDir={#OutDir}
OutputBaseFilename=RadioMoon-{#AppVersion}-windows-{#Arch}-setup
SetupIconFile={#SourcePath}..\packaging\icon.ico
UninstallDisplayIcon={app}\RadioMoon.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
#if Arch == "x64"
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#elif Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Dirs]
; data folder shared by all users of the PC (database, backups, settings)
Name: "{commonappdata}\RadioMoon"; Permissions: users-modify

[Icons]
Name: "{group}\RadioMoon"; Filename: "{app}\RadioMoon.exe"
Name: "{group}\RadioMoon MPX Receiver"; Filename: "{app}\RadioMoon-MPX-Receiver.exe"
Name: "{autodesktop}\RadioMoon"; Filename: "{app}\RadioMoon.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\RadioMoon.exe"; Description: "{cm:LaunchProgram,RadioMoon}"; Flags: nowait postinstall skipifsilent
; The database, backups and settings live in %PROGRAMDATA%\RadioMoon and are kept when the program is uninstalled or upgraded.
