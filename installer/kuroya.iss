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
DisableProgramGroupPage=yes
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
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a Desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: checkedonce
Name: "startmenuicon"; Description: "Create a Start Menu shortcut"; GroupDescription: "Shortcuts:"; Flags: checkedonce
Name: "taskbarpin"; Description: "Attempt to pin Kuroya to the taskbar"; GroupDescription: "Shortcuts:"

[Files]
Source: "{#SourceRoot}\target\release\kuroya.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceRoot}\installer\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceRoot}\installer\pin-taskbar.ps1"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Kuroya\Kuroya"; Filename: "{app}\kuroya.exe"; WorkingDir: "{app}"; IconFilename: "{app}\kuroya.exe"; Tasks: startmenuicon
Name: "{userdesktop}\Kuroya"; Filename: "{app}\kuroya.exe"; WorkingDir: "{app}"; IconFilename: "{app}\kuroya.exe"; Tasks: desktopicon

[Run]
Filename: "powershell.exe"; Parameters: "-ExecutionPolicy Bypass -NoProfile -WindowStyle Hidden -File ""{app}\pin-taskbar.ps1"" ""{userprograms}\Kuroya\Kuroya.lnk"""; Flags: runhidden; Tasks: taskbarpin
Filename: "{app}\kuroya.exe"; Description: "Launch Kuroya"; Flags: nowait postinstall skipifsilent unchecked

[UninstallDelete]
Type: filesandordirs; Name: "{userprograms}\Kuroya"
Type: files; Name: "{userdesktop}\Kuroya.lnk"
Type: files; Name: "{userappdata}\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\Kuroya.lnk"
