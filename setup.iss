; Inno Setup script for Lecture Saver
; Expects: dist\LectureSaver.exe (from PyInstaller) and ffmpeg\ffmpeg.exe (see README)

[Setup]
AppName=Lecture Saver
AppVersion=1.0
DefaultDirName={autopf}\LectureSaver
DefaultGroupName=Lecture Saver
OutputBaseFilename=LectureSaverSetup
Compression=lzma
SolidCompression=yes
PrivilegesRequired=lowest

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "dist\LectureSaver.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "ffmpeg\ffmpeg.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "ffmpeg\LICENSE*"; DestDir: "{app}\licenses"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\Lecture Saver"; Filename: "{app}\LectureSaver.exe"
Name: "{autodesktop}\Lecture Saver"; Filename: "{app}\LectureSaver.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\LectureSaver.exe"; Description: "Launch Lecture Saver"; Flags: postinstall nowait skipifsilent
