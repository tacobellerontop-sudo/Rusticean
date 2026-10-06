; Rusticean installer (Inno Setup 6). Built by CI:
;   cargo build --release
;   iscc /DAppVersion=0.1.0 installer\Rusticean.iss
; Installs per user (no admin prompt) to %LOCALAPPDATA%\Programs\Rusticean. Downloaded
; Roblox versions, settings and logs live separately in %LOCALAPPDATA%\Rusticean.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6E2F5B1A-3C7D-4E8B-9A41-5F0D2C7B8E13}
AppName=Rusticean
AppVersion={#AppVersion}
AppVerName=Rusticean {#AppVersion}
AppPublisher=Rusticean
AppPublisherURL=https://github.com/tacobellerontop-sudo/fictional-garbanzo
DefaultDirName={localappdata}\Programs\Rusticean
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\target\installer
OutputBaseFilename=Rusticean-Setup
SetupIconFile=..\assets\rusticean.ico
UninstallDisplayIcon={app}\Rusticean.exe
UninstallDisplayName=Rusticean
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
CloseApplications=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "..\target\release\Rusticean.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Rusticean"; Filename: "{app}\Rusticean.exe"
Name: "{autodesktop}\Rusticean"; Filename: "{app}\Rusticean.exe"; Tasks: desktopicon

[Run]
; make roblox:// links open Rusticean straight away
Filename: "{app}\Rusticean.exe"; Parameters: "-register"; Flags: runhidden
Filename: "{app}\Rusticean.exe"; Description: "Open Rusticean"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\Rusticean.exe"; Parameters: "-unregister"; Flags: runhidden; RunOnceId: "Unregister"

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{localappdata}\Rusticean');
    if DirExists(DataDir) and not UninstallSilent then
      if MsgBox('Also delete Rusticean''s data (installed Roblox versions, settings, mods and logs)?',
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir, True, True, True);
  end;
end;
