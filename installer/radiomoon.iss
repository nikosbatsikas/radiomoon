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
Name: "dbservice"; Description: "Install the database server (MariaDB) as a Windows service that starts with Windows"; GroupDescription: "Database:"; Check: HasBundledDb

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[InstallDelete]
; the MPX programs moved to the RadioMoon MPX setup (github.com/nikosbatsikas/radiomoon-mpx)
Type: files; Name: "{app}\RadioMoon-MPX-Receiver.exe"
Type: files; Name: "{app}\RadioMoon-MPX-Transmitter.exe"
Type: files; Name: "{group}\RadioMoon MPX Receiver.lnk"
Type: files; Name: "{group}\RadioMoon MPX Transmitter.lnk"

[Dirs]
; data folder shared by all users of the PC (database, backups, settings)
Name: "{commonappdata}\RadioMoon"; Permissions: users-modify

[Icons]
Name: "{group}\RadioMoon"; Filename: "{app}\RadioMoon.exe"
Name: "{group}\RadioMoon Client (connect to a server)"; Filename: "{app}\RadioMoon-Client.exe"
Name: "{autodesktop}\RadioMoon"; Filename: "{app}\RadioMoon.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\RadioMoon.exe"; Description: "{cm:LaunchProgram,RadioMoon}"; Flags: nowait postinstall skipifsilent
; The database, backups and settings live in %PROGRAMDATA%\RadioMoon and are kept when the program is uninstalled or upgraded.

[Code]
const
  SvcName = 'RadioMoonDB';

function HasBundledDb: Boolean;
begin
  Result := FileExists('{#SourceDir}\mariadb\bin\mysqld.exe');
end;

function RunHidden(const F, P: String): Integer;
var rc: Integer;
begin
  if not Exec(F, P, '', SW_HIDE, ewWaitUntilTerminated, rc) then rc := -1;
  Result := rc;
end;

function ServiceExists: Boolean;
begin
  Result := RunHidden(ExpandConstant('{sys}\sc.exe'), 'query ' + SvcName) = 0;
end;

// stop the database service while the files are replaced (upgrade)
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  if ServiceExists then
    RunHidden(ExpandConstant('{sys}\net.exe'), 'stop ' + SvcName);
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var bin, data, ini: String;
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('dbservice') then
  begin
    bin := ExpandConstant('{app}\mariadb\bin');
    data := ExpandConstant('{commonappdata}\RadioMoon\data');
    ini := data + '\my.ini';
    ForceDirectories(data);
    // first installation: create the database folder (no password, local connections only - same as the program expects)
    if not DirExists(data + '\mysql') then
      RunHidden(bin + '\mysql_install_db.exe', '--datadir="' + data + '" --port=3307 --silent');
    SetIniString('mysqld', 'datadir', data, ini);
    SetIniString('mysqld', 'port', '3307', ini);
    SetIniString('mysqld', 'bind-address', '127.0.0.1', ini);
    SetIniString('mysqld', 'character-set-server', 'utf8mb4', ini);
    SetIniString('mysqld', 'collation-server', 'utf8mb4_unicode_ci', ini);
    SetIniString('mysqld', 'innodb_buffer_pool_size', '64M', ini);
    if not ServiceExists then
      RunHidden(bin + '\mysqld.exe', '--install ' + SvcName + ' --defaults-file="' + ini + '"');
    RunHidden(ExpandConstant('{sys}\sc.exe'), 'config ' + SvcName + ' start= auto');
    RunHidden(ExpandConstant('{sys}\net.exe'), 'start ' + SvcName);
  end;
end;

// removing the program removes the service but keeps the data (ProgramData\RadioMoon)
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    if ServiceExists then
    begin
      RunHidden(ExpandConstant('{sys}\net.exe'), 'stop ' + SvcName);
      RunHidden(ExpandConstant('{sys}\sc.exe'), 'delete ' + SvcName);
    end;
end;
