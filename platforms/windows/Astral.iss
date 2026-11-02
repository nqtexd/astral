#ifndef AppVersion
  #define AppVersion "0.15.0"
#endif
#ifndef PackageDir
  #define PackageDir "..\..\release\Astral-" + AppVersion + "-windows-x64"
#endif
[Setup]
AppId={{751F1D8E-9C57-4FE0-B527-66DE3A139A48}
AppName=Astral
AppVersion={#AppVersion}
AppPublisher=Astral contributors
AppPublisherURL=https://github.com/nqtexd/astral
DefaultDirName={localappdata}\Programs\Astral
DefaultGroupName=Astral
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
LicenseFile=..\..\COPYING
SetupIconFile=..\..\assets\brand\astral.ico
UninstallDisplayIcon={app}\Astral-Studio.exe
OutputDir=..\..\release
OutputBaseFilename=Astral-{#AppVersion}-Setup-windows-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
ChangesAssociations=yes
[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{group}\Astral"; Filename: "{app}\Astral-Studio.exe"
Name: "{autodesktop}\Astral"; Filename: "{app}\Astral-Studio.exe"; Tasks: desktopicon
[Run]
Filename: "{app}\Astral-Studio.exe"; Description: "Launch Astral"; Flags: nowait postinstall skipifsilent
; User projects, preferences and API credentials live outside {app} and are never removed.
