; Inno Setup script for bbbstudentDownloader
; Expects: dist\bbbStudentDownloader.exe (from PyInstaller) and ffmpeg\ffmpeg.exe (see README)

[Setup]
AppName=bbbstudentDownloader
AppVersion=1.0
DefaultDirName={autopf}\bbbStudentDownloader
DefaultGroupName=bbbstudentDownloader
OutputBaseFilename=bbbStudentDownloaderSetup
Compression=lzma
SolidCompression=yes
PrivilegesRequired=lowest

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "dist\bbbStudentDownloader.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "ffmpeg\ffmpeg.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "ffmpeg\LICENSE*"; DestDir: "{app}\licenses"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\bbbstudentDownloader"; Filename: "{app}\bbbStudentDownloader.exe"
Name: "{autodesktop}\bbbstudentDownloader"; Filename: "{app}\bbbStudentDownloader.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\bbbStudentDownloader.exe"; Description: "Launch bbbstudentDownloader"; Flags: postinstall nowait skipifsilent
