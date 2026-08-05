; Wixen Uninstaller — Inno Setup Script
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

#define AppName      "Wixen Uninstaller"
#define AppVersion   "0.1.0"
#define AppPublisher "PratikP1"
#define AppURL       "https://github.com/PratikP1/Wixen-Uninstall"
#define AppExeName   "wixen_uninstall.exe"
#define BinaryDir    "..\target\release"

[Setup]
AppId={{A7C3D2F1-8E4B-4C9A-B5D6-1F2E3A4B5C6D}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
UninstallDisplayIcon={app}\{#AppExeName}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputBaseFilename=WixenUninstaller-Setup-{#AppVersion}
OutputDir=.\installer_output
Compression=lzma2/ultra
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
; Explicitly target supported Windows releases and 64-bit installs.
MinVersion=10.0
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

; Require Administrator — the uninstaller must run elevated.
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=commandline

; Accessibility: keep the wizard simple and screen-reader friendly.
; AppReadmeFile points to the README so it is shown in the wizard.
AppReadmeFile=..\README.md

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
; The compiled binary.
Source: "{#BinaryDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion

; Human-readable documentation.
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Icons]
; Start-menu shortcut — runs elevated automatically.
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"; \
    Parameters: ""; \
    WorkingDir: "{app}"; \
    Comment: "Remove stubborn antivirus suites completely"

Name: "{userdesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; \
    Tasks: desktopicon; \
    Comment: "Remove stubborn antivirus suites completely"

[Run]
; Offer to launch immediately after install.
Filename: "{app}\{#AppExeName}"; \
    Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; \
    Flags: nowait postinstall skipifsilent runascurrentuser

[UninstallRun]
; Nothing extra to do — the Windows uninstaller entry is auto-created by Inno.

[UninstallDelete]
Type: dirifempty; Name: "{app}"

[Code]
{ ---------------------------------------------------------------------------- }
{ Pascal script — accessibility helpers                                        }
{ ---------------------------------------------------------------------------- }

{ Announce wizard page changes to the Windows accessibility framework via      }
{ WM_GETOBJECT so screen readers like NVDA and JAWS receive focus events.      }
procedure CurPageChanged(CurPageID: Integer);
begin
  { The default Inno Setup wizard already sends WM_ACTIVATEAPP messages which  }
  { most screen readers pick up; no extra script required for basic operation. }
  { If you need custom announcements, call NotifyAccessibilityEvent() here.    }
end;
