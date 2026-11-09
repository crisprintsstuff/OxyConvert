; Inno Setup script for the OxyConvert Windows installer.
;
; Built by:
;   iscc /DAppVersion=0.1.0 /DNumericVersion=0.1.0.0 /DSourceDir=<dir with oxyconvert.exe> packaging\windows\oxyconvert.iss
;
; Installs per-user by default; registers .oxyaddon file association.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef NumericVersion
  #define NumericVersion "0.1.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\target\x86_64-pc-windows-msvc\release"
#endif
#define RepoRoot "..\.."

[Setup]
AppId={{91AB42E8-89DF-42A1-94E5-B5E3C8B720E9}
AppName=OxyConvert
AppVersion={#AppVersion}
AppVerName=OxyConvert {#AppVersion}
AppPublisher=OxyLabs
AppPublisherURL=https://github.com/crisprintsstuff/OxyConvert
AppSupportURL=https://github.com/crisprintsstuff/OxyConvert/issues
VersionInfoVersion={#NumericVersion}
VersionInfoProductName=OxyConvert
VersionInfoDescription=OxyConvert installer
DefaultDirName={autopf}\OxyConvert
DefaultGroupName=OxyConvert
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
ChangesAssociations=yes
UninstallDisplayIcon={app}\oxyconvert.exe
UninstallDisplayName=OxyConvert {#AppVersion}
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir={#RepoRoot}\dist
OutputBaseFilename=OxyConvert-{#AppVersion}-windows-x86_64-setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associate"; Description: "Associate .oxyaddon files with OxyConvert"; GroupDescription: "File associations:"

[Files]
Source: "{#SourceDir}\oxyconvert.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\SPEC_OXYADDON.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\OxyConvert"; Filename: "{app}\oxyconvert.exe"
Name: "{autodesktop}\OxyConvert"; Filename: "{app}\oxyconvert.exe"; Tasks: desktopicon

[Registry]
; .oxyaddon association
Root: HKA; Subkey: "Software\Classes\.oxyaddon\OpenWithProgids"; ValueType: string; ValueName: "OxyConvert.Package"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\OxyConvert.Package"; ValueType: string; ValueName: ""; ValueData: "OxyBrush Addon Package"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\OxyConvert.Package\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\oxyconvert.exe,0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\OxyConvert.Package\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\oxyconvert.exe"" inspect ""%1"""; Tasks: associate
Root: HKA; Subkey: "Software\Classes\OxyConvert.Package\shell\Install to OxyBrush\command"; ValueType: string; ValueName: ""; ValueData: """{app}\oxyconvert.exe"" install ""%1"""; Tasks: associate

[Run]
Filename: "{app}\oxyconvert.exe"; Description: "{cm:LaunchProgram,OxyConvert}"; Flags: nowait postinstall skipifsilent
