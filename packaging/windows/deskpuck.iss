;   ISCC.exe /DVersion=0.3.0 /DSource=<staged folder> /DOutput=<folder> deskpuck.iss

#ifndef Version
  #error Pass /DVersion=x.y.z
#endif
#ifndef Source
  #error Pass /DSource=<staged release folder>
#endif
#ifndef Output
  #error Pass /DOutput=<folder for the installer>
#endif

[Setup]
; Fixed for good: Windows matches upgrades and the uninstaller by this id.
AppId={{F0FA84CD-5402-434B-9A3E-9C18C110CD17}
AppName=Deskpuck
AppVersion={#Version}
AppVerName=Deskpuck {#Version}
AppPublisher=Pyronewbic
AppPublisherURL=https://github.com/Pyronewbic/deskpuck
AppSupportURL=https://github.com/Pyronewbic/deskpuck/issues
DefaultDirName={autopf}\Deskpuck
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#Output}
OutputBaseFilename=Deskpuck-{#Version}-setup
SetupIconFile=..\..\rust\icons\deskpuck.ico
UninstallDisplayIcon={app}\deskpuck.exe
UninstallDisplayName=Deskpuck
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "startatlogin"; Description: "Start Deskpuck when I log in"; Flags: unchecked

[Files]
Source: "{#Source}\deskpuck.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\deskpuck-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\README.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Deskpuck"; Filename: "{app}\deskpuck.exe"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; \
    ValueName: "Deskpuck"; ValueData: """{app}\deskpuck.exe"""; Tasks: startatlogin

[Run]
Filename: "{app}\deskpuck.exe"; Description: "Start Deskpuck"; Flags: nowait postinstall skipifsilent

[Code]
// The tray can also set Start at Login, so remove the Run value whoever wrote it.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Deskpuck');
end;
