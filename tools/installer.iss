; The Windows installer of the released game: first `cargo build --release -p tropa` and
; `python -X utf8 tools/pack_assets.py desktop` (the assets with nothing lost, about half the size,
; only what the game loads), then
;   ISCC.exe tools\installer.iss
; It comes out as target\dist\TropaVNav-Setup-<version>.exe.
#ifndef Version
  #define Version "0.1.8"
#endif

[Setup]
AppId=TropaVNav
AppName={cm:GameName}
AppVersion={#Version}
AppPublisher={cm:GameName}
DefaultDirName={autopf}\TropaVNav
DefaultGroupName={cm:GameName}
OutputDir=..\target\dist
OutputBaseFilename=TropaVNav-Setup-{#Version}
UninstallDisplayIcon={app}\tropa.ico
SetupIconFile=icon.ico
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
; pictures and sounds are already packed: more effort only makes the build slower
Compression=lzma2/fast
SolidCompression=no
DiskSpanning=no
WizardStyle=modern
PrivilegesRequired=admin

[Languages]
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "ruold"; MessagesFile: "compiler:Languages\Russian.isl,ruold.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
ru.GameName=Тропа в Навь
ruold.GameName=Тропа въ Навь
en.GameName=Path to Nav
ru.Firewall=Разрешить игру по локальной сети и Radmin VPN (правило брандмауэра)
ruold.Firewall=Разрешить игру по локальной сети и Radmin VPN (правило брандмауэра)
en.Firewall=Allow play over a local network or Radmin VPN (firewall rule)
ru.Network=Сеть:
ruold.Network=Сеть:
en.Network=Network:

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "firewall"; Description: "{cm:Firewall}"; GroupDescription: "{cm:Network}"

[Files]
Source: "..\target\release\tropa.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\steam_api64.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "icon.ico"; DestDir: "{app}"; DestName: "tropa.ico"; Flags: ignoreversion
Source: "..\target\pack\desktop\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{cm:GameName}"; Filename: "{app}\tropa.exe"; IconFilename: "{app}\tropa.ico"
Name: "{group}\{cm:UninstallProgram,{cm:GameName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{cm:GameName}"; Filename: "{app}\tropa.exe"; IconFilename: "{app}\tropa.ico"; Tasks: desktopicon

[Run]
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall add rule name=""Tropa v Nav"" dir=in action=allow program=""{app}\tropa.exe"" enable=yes"; Flags: runhidden; Tasks: firewall
Filename: "{app}\tropa.exe"; Description: "{cm:LaunchProgram,{cm:GameName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Tropa v Nav"""; Flags: runhidden; RunOnceId: "TropaFirewall"

[UninstallDelete]
Type: files; Name: "{app}\assets\install_lang.txt"

[Code]
// the game starts in the language it was installed in, until the player chooses another in it
procedure CurStepChanged(CurStep: TSetupStep);
var
  Word: String;
begin
  if CurStep = ssPostInstall then
  begin
    Word := ActiveLanguage;
    if Word = 'ruold' then Word := 'old';
    SaveStringToFile(ExpandConstant('{app}\assets\install_lang.txt'), Word, False);
  end;
end;
