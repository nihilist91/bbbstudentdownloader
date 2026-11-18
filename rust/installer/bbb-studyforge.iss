; Inno Setup script for BBB StudyForge (Rust build).
;
; Build after `cargo build --release --features gui`, from the rust/ folder:
;   iscc /DAppVersion=2.4.0 installer\bbb-studyforge.iss
; Expects:
;   target\release\bbb-studyforge.exe
;   target\release\bbb-studyforge-gui.exe
;   dist\ffmpeg\ffmpeg.exe   (optional; the app can download FFmpeg itself)
; Output: installer\Output\bbb-studyforge-<version>-setup.exe

#ifndef AppVersion
  #define AppVersion "2.4.0"
#endif

#define AppName "BBB StudyForge"
#define AppPublisher "THE NIHILIST"
#define AppURL "https://github.com/nihilist91/bbbstudentdownloader"
#define GuiExe "bbb-studyforge-gui.exe"
#define CliExe "bbb-studyforge.exe"

[Setup]
; Separate AppId from the Python installer, so both can be installed side by side.
AppId={{A8818F7C-0872-468B-93BE-38B38C6B1868}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
SetupIconFile=..\..\assets\app_icon.ico
UninstallDisplayIcon={app}\{#GuiExe}
UninstallDisplayName={#AppName}
OutputDir=Output
OutputBaseFilename=bbb-studyforge-{#AppVersion}-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
; Per-user install by default (no admin prompt, and the app can write ffmpeg next to itself).
; The user can still choose "Install for all users" in the first dialog.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "addtopath"; Description: "Add the command-line tool (bbb-studyforge) to PATH"; GroupDescription: "Command line:"; Flags: unchecked

[Files]
Source: "..\target\release\{#GuiExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\{#CliExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\ffmpeg\ffmpeg.exe"; DestDir: "{app}\ffmpeg"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\dist\ffmpeg\LICENSE*"; DestDir: "{app}\ffmpeg"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\README.md"; DestDir: "{app}"; DestName: "README.md"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#GuiExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#GuiExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#GuiExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: postinstall nowait skipifsilent

[UninstallDelete]
; FFmpeg downloaded later by the app's "Install FFmpeg" button.
Type: filesandordirs; Name: "{app}\ffmpeg"

[Code]
const
  UserEnvKey = 'Environment';
  SystemEnvKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

function EnvRoot: Integer;
begin
  if IsAdminInstallMode then Result := HKEY_LOCAL_MACHINE else Result := HKEY_CURRENT_USER;
end;

function EnvKey: String;
begin
  if IsAdminInstallMode then Result := SystemEnvKey else Result := UserEnvKey;
end;

{ True when Dir is one of the ';'-separated entries in PathValue (case-insensitive). }
function PathHasEntry(const PathValue, Dir: String): Boolean;
begin
  Result := Pos(';' + Uppercase(RemoveBackslashUnlessRoot(Dir)) + ';',
                ';' + Uppercase(PathValue) + ';') > 0;
end;

procedure AddAppToPath;
var
  Paths: String;
begin
  if not RegQueryStringValue(EnvRoot, EnvKey, 'Path', Paths) then Paths := '';
  if PathHasEntry(Paths, ExpandConstant('{app}')) then exit;
  if (Paths <> '') and (Copy(Paths, Length(Paths), 1) <> ';') then Paths := Paths + ';';
  Paths := Paths + ExpandConstant('{app}');
  RegWriteExpandStringValue(EnvRoot, EnvKey, 'Path', Paths);
end;

procedure RemoveAppFromPath;
var
  Paths, Entry, Rest, Kept, Dir: String;
  P: Integer;
begin
  if not RegQueryStringValue(EnvRoot, EnvKey, 'Path', Paths) then exit;
  Dir := Uppercase(RemoveBackslashUnlessRoot(ExpandConstant('{app}')));
  Rest := Paths;
  Kept := '';
  while Rest <> '' do
  begin
    P := Pos(';', Rest);
    if P = 0 then
    begin
      Entry := Rest;
      Rest := '';
    end else begin
      Entry := Copy(Rest, 1, P - 1);
      Rest := Copy(Rest, P + 1, Length(Rest));
    end;
    if (Entry <> '') and (Uppercase(RemoveBackslashUnlessRoot(Entry)) <> Dir) then
    begin
      if Kept <> '' then Kept := Kept + ';';
      Kept := Kept + Entry;
    end;
  end;
  if Kept <> Paths then RegWriteExpandStringValue(EnvRoot, EnvKey, 'Path', Kept);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then AddAppToPath;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  { Always clean up, whether or not PATH was added: removing an absent entry does nothing. }
  if CurUninstallStep = usPostUninstall then RemoveAppFromPath;
end;
