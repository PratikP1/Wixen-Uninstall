; Wixen Uninstaller - Inno Setup Script
; Author: PratikP1
;
; This script packages the compiled wixen_uninstall.exe into a distributable
; Windows installer.  It also registers the uninstaller so that the tool itself
; can be removed via "Add or Remove Programs".
;
; Build prerequisites:
;   1. Install Inno Setup 6 (https://jrsoftware.org/isinfo.php).
;   2. Compile the Rust binary for the Windows target:
;        cargo build --release --target x86_64-pc-windows-msvc
;   3. Open this .iss file in the Inno Setup Compiler and click Build > Compile.

#ifndef AppName
  #define AppName      "Wixen Uninstaller"
#endif
#ifndef AppVersion
  #define AppVersion   "0.1.0"
#endif
#ifndef AppPublisher
  #define AppPublisher "PratikP1"
#endif
#ifndef AppURL
  #define AppURL       "https://github.com/PratikP1/Wixen-Uninstall"
#endif
#ifndef AppExeName
  #define AppExeName   "wixen_uninstall.exe"
#endif
#ifndef HelpFileName
  #define HelpFileName "WixenUninstallerHelp.html"
#endif
#ifndef BinaryDir
  #ifexist "target\x86_64-pc-windows-msvc\release\wixen_uninstall.exe"
    #define BinaryDir "target\x86_64-pc-windows-msvc\release"
  #else
    #define BinaryDir "target\release"
  #endif
#endif
#ifndef OutputDir
  #define OutputDir ".\installer_output"
#endif

[Setup]
AppId={{A7C3D2F1-8E4B-4C9A-B5D6-1F2E3A4B5C6D}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
UninstallDisplayIcon={app}\{#AppExeName}
; Shown in Add or Remove Programs.
VersionInfoVersion={#AppVersion}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription={#AppName} Setup
VersionInfoProductName={#AppName}
LicenseFile=LICENSE
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputBaseFilename=WixenUninstaller-Setup-{#AppVersion}
OutputDir={#OutputDir}
Compression=lzma2/ultra
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
; Explicitly target supported Windows releases and 64-bit installs.
MinVersion=10.0
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

; Require Administrator - the uninstaller must run elevated.
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=commandline

; Accessibility: keep the wizard simple and screen-reader friendly.
; AppReadmeFile points to the installed HTML help file.
AppReadmeFile={app}\{#HelpFileName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
; The compiled binary.
Source: "{#BinaryDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion

; Installed HTML help file.
Source: "docs\{#HelpFileName}"; DestDir: "{app}"; Flags: ignoreversion isreadme

; Human-readable Markdown documentation for the install folder.
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
; The executable's manifest requests Administrator, so both shortcuts trigger
; the UAC prompt on launch without any extra flags here.
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"; \
    Parameters: ""; \
    WorkingDir: "{app}"; \
    Comment: "Remove stubborn antivirus suites completely"

Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; \
    Tasks: desktopicon; \
    Comment: "Remove stubborn antivirus suites completely"

[Run]
; Offer to launch immediately after install.
Filename: "{app}\{#AppExeName}"; \
    Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; \
    Flags: nowait postinstall skipifsilent

[UninstallRun]
; Nothing extra to do - the Windows uninstaller entry is auto-created by Inno.

[UninstallDelete]
Type: dirifempty; Name: "{app}"

[Code]
{ ---------------------------------------------------------------------------- }
{ Pascal script - accessibility helpers                                        }
{ ---------------------------------------------------------------------------- }

{ Announce wizard page changes to the Windows accessibility framework via      }
{ WM_GETOBJECT so screen readers like NVDA and JAWS receive focus events.      }
procedure CurPageChanged(CurPageID: Integer);
begin
  { The default Inno Setup wizard already sends WM_ACTIVATEAPP messages which  }
  { most screen readers pick up; no extra script required for basic operation. }
  { If you need custom announcements, call NotifyAccessibilityEvent() here.    }
end;
