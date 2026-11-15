; Machine installer: same product path/key, retained Go rollback.
Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
!include "WinVer.nsh"
!ifndef CONTENT
  !error "Verified product CONTENT required"
!endif
!ifndef OUTPUT
  !error "OUTPUT required"
!endif
!ifndef VERSION
  !error "VERSION required"
!endif
!ifndef RENDERER
  !error "RENDERER required"
!endif
Name "Velo"
OutFile "${OUTPUT}"
InstallDir "$PROGRAMFILES64\HBLADEH\Velo"
InstallDirRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVelo" "InstallLocation"
RequestExecutionLevel admin
ManifestDPIAware true
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Velo"
VIAddVersionKey "FileDescription" "Velo 安装程序"
VIAddVersionKey "Comments" "Renderer: ${RENDERER}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "CompanyName" "HBLADEH"
VIAddVersionKey "LegalCopyright" "Copyright (c) HBLADEH"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
Var PowerShell
Var Parameters
Var Option
Var Operation
Function .onInit
  SetRegView 64
  ${IfNot} ${IsNativeAMD64}
    SetErrorLevel 20
    Quit
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    SetErrorLevel 20
    Quit
  ${EndIf}
  ; The official Go registration predates InstallLocation.
  ReadRegStr $0 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVelo" "UninstallString"
  ${If} $0 != ""
    StrCpy $1 $0 1
    ${If} $1 == '$\"'
      StrCpy $0 $0 -1 1
      ${GetParent} $0 $INSTDIR
    ${EndIf}
  ${EndIf}
FunctionEnd
!macro SetPowerShell
  ; PowerShell 7 callers can export incompatible module paths to Windows
  ; PowerShell 5.1. Let the child initialize its own built-in modules.
  System::Call 'kernel32::SetEnvironmentVariableW(w "PSModulePath", p 0)'
  StrCpy $PowerShell "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  ${If} ${RunningX64}
    StrCpy $PowerShell "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  ${EndIf}
!macroend
Section "Install"
  InitPluginsDir
  SetOutPath "$PLUGINSDIR\payload"
  ClearErrors
  File /r "${CONTENT}\*"
  WriteUninstaller "$PLUGINSDIR\payload\uninstall.exe"
  ${If} ${Errors}
    SetErrorLevel 22
    Quit
  ${EndIf}
  !insertmacro SetPowerShell
  nsExec::ExecToLog '$\"$PowerShell$\" -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File $\"$PLUGINSDIR\payload\machine-install.ps1$\" -Action Install -InstallDirectory $\"$INSTDIR$\" -Payload $\"$PLUGINSDIR\payload$\"'
  Pop $0
  ${If} $0 == 33
    SetErrorLevel 33
    IfSilent +2
      MessageBox MB_OK|MB_ICONSTOP "另一个 Velo 安装或卸载正在进行，请等待完成后再试。"
    Quit
  ${EndIf}
  ${If} $0 != 0
    SetErrorLevel 30
    IfSilent +2
      MessageBox MB_OK|MB_ICONSTOP "安装未完成。请先退出 Velo；旧程序或恢复记录已保留。查看安装目录 .velo-rollback 中的诊断后重试。"
    Quit
  ${EndIf}
  SetErrorLevel 0
SectionEnd
Section "Uninstall"
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=machine-install.ps1 "${CONTENT}\machine-install.ps1"
  StrCpy $Operation "Uninstall"
  ${GetParameters} $Parameters
  ClearErrors
  ${GetOptions} $Parameters "/ROLLBACK" $Option
  ${IfNot} ${Errors}
    StrCpy $Operation "Rollback"
  ${Else}
    IfFileExists "$INSTDIR\.velo-rollback\original-go\snapshot.json" 0 no_choice
    IfSilent no_choice
    MessageBox MB_YESNOCANCEL|MB_ICONQUESTION "是否回退至安装前保留的 Go 版本？$\r$\n是：回退 Go（保留当前用户数据）。$\r$\n否：卸载 Rust（保留用户数据与程序恢复档案）。" IDYES rollback IDNO no_choice
    SetErrorLevel 1
    Quit
    rollback:
      StrCpy $Operation "Rollback"
    no_choice:
  ${EndIf}
  !insertmacro SetPowerShell
  nsExec::ExecToLog '$\"$PowerShell$\" -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File $\"$PLUGINSDIR\machine-install.ps1$\" -Action $Operation -InstallDirectory $\"$INSTDIR$\"'
  Pop $0
  ${If} $0 == 33
    SetErrorLevel 33
    IfSilent +2
      MessageBox MB_OK|MB_ICONSTOP "另一个 Velo 安装或卸载正在进行，请等待完成后再试。"
    Quit
  ${EndIf}
  ${If} $0 != 0
    SetErrorLevel 30
    IfSilent +2
      MessageBox MB_OK|MB_ICONSTOP "操作未完成。请先退出 Velo；程序和用户数据恢复记录已保留。"
    Quit
  ${EndIf}
  SetErrorLevel 0
SectionEnd
