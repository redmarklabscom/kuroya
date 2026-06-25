#ifndef SourceRoot
  #error SourceRoot is required
#endif

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
AppId={{B7ADF221-E903-4075-8A67-2DE905EF5A31}
AppName=Kuroya
AppVersion={#AppVersion}
AppVerName=Kuroya {#AppVersion}
AppPublisher=Kuroya Contributors
AppPublisherURL=https://github.com/redmarklabscom/kuroya
AppSupportURL=https://github.com/redmarklabscom/kuroya/issues
AppUpdatesURL=https://github.com/redmarklabscom/kuroya/releases
AppCopyright=Copyright 2026 Kuroya Contributors
VersionInfoVersion={#AppVersion}
VersionInfoCompany=Kuroya Contributors
VersionInfoDescription=Kuroya Setup
VersionInfoProductName=Kuroya
DefaultDirName={localappdata}\Programs\Kuroya
DisableDirPage=yes
DisableProgramGroupPage=yes
UsePreviousAppDir=yes
LicenseFile={#SourceRoot}\installer\LICENSE.txt
OutputDir={#SourceRoot}\dist
OutputBaseFilename=Kuroya-Setup-{#AppVersion}
SetupIconFile={#SourceRoot}\assets\logos\kuroya.ico
UninstallDisplayName=Kuroya
UninstallDisplayIcon={app}\kuroya.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
CloseApplications=yes
RestartApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a Desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: checkedonce
Name: "startmenuicon"; Description: "Create a Start Menu shortcut"; GroupDescription: "Shortcuts:"; Flags: checkedonce

[Files]
Source: "{#SourceRoot}\target\release\kuroya.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceRoot}\installer\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Kuroya\Kuroya"; Filename: "{app}\kuroya.exe"; WorkingDir: "{app}"; IconFilename: "{app}\kuroya.exe"; Tasks: startmenuicon
Name: "{userdesktop}\Kuroya"; Filename: "{app}\kuroya.exe"; WorkingDir: "{app}"; IconFilename: "{app}\kuroya.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\kuroya.exe"; Description: "Launch Kuroya"; Flags: nowait postinstall skipifsilent unchecked
Filename: "{app}\kuroya.exe"; Flags: nowait skipifdoesntexist; Check: ShouldRestartKuroyaAfterUpdate

[UninstallDelete]
Type: filesandordirs; Name: "{userprograms}\Kuroya"
Type: files; Name: "{userdesktop}\Kuroya.lnk"

[Code]
function ShouldRestartKuroyaAfterUpdate: Boolean;
begin
  Result := ExpandConstant('{param:KuroyaRestart|0}') = '1';
end;
