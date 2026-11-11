; Inno Setup script for BBB StudyForge (bbbStudentDownloader 2.0)
; Expects: dist\bbbStudentDownloader.exe (from PyInstaller) and ffmpeg\ffmpeg.exe (see README)

[Setup]
AppName=BBB StudyForge
AppVersion=2.0
DefaultDirName={autopf}\bbbStudentDownloader
DefaultGroupName=BBB StudyForge
OutputBaseFilename=bbbStudentDownloaderSetup
Compression=lzma
SolidCompression=yes
PrivilegesRequired=lowest

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "dist\bbbStudentDownloader.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "ffmpeg\ffmpeg.exe"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "ffmpeg\LICENSE*"; DestDir: "{app}\licenses"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\BBB StudyForge"; Filename: "{app}\bbbStudentDownloader.exe"
Name: "{autodesktop}\BBB StudyForge"; Filename: "{app}\bbbStudentDownloader.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\bbbStudentDownloader.exe"; Description: "Launch BBB StudyForge"; Flags: postinstall nowait skipifsilent
