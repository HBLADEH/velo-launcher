; Local per-user preview installer. It never claims a public release or G4.
Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
!ifndef CONTENT
  !error "Verified preview CONTENT directory is required"
!endif
!ifndef OUTPUT
  !error "OUTPUT is required"
!endif
!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef RENDERER
  !error "RENDERER is required"
!endif
Name "Velo Rust Preview (${RENDERER})"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\VeloSlintPreview"
RequestExecutionLevel user
ManifestDPIAware true
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Velo Rust Preview"
VIAddVersionKey "FileDescription" "Velo Rust Preview ${RENDERER} installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "CompanyName" "HBLADEH"
VIAddVersionKey "LegalCopyright" "Copyright (c) HBLADEH"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!ifdef VALIDATION_INSTALL_DIR
  Var Parameters
  Var FailAfterBackup
!endif
Var HadPrevious

Function .onInit
  ${IfNot} ${RunningX64}
    SetErrorLevel 20
    Quit
  ${EndIf}
  SetShellVarContext current
  SetRegView 64
  System::Call 'kernel32::OpenMutexW(i 0x00100000, i 0, w "Local\VeloSlint.Instance.preview") p.r0'
  ${If} $0 != 0
    System::Call 'kernel32::CloseHandle(p r0)'
    SetErrorLevel 23
    Quit
  ${EndIf}
  !ifdef VALIDATION_INSTALL_DIR
    ; Tests compile the same installation logic with all writes constrained to
    ; one explicit directory. No Start Menu, desktop or uninstall registry.
    StrCmp $INSTDIR "${VALIDATION_INSTALL_DIR}" +3
      SetErrorLevel 21
      Quit
  !endif
FunctionEnd

Function un.onInit
  SetShellVarContext current
  SetRegView 64
  !ifdef VALIDATION_INSTALL_DIR
    StrCmp $INSTDIR "${VALIDATION_INSTALL_DIR}" +3
      SetErrorLevel 21
      Quit
  !endif
  IfFileExists "$INSTDIR\preview-owner.txt" +3
    SetErrorLevel 25
    Quit
  ; An executable mapped by a running process cannot be opened for deletion.
  ; Refuse before removing licenses, shortcuts or the uninstall registration.
  IfFileExists "$INSTDIR\velo-slint.exe" 0 unlocked
    System::Call 'kernel32::CreateFileW(w "$INSTDIR\velo-slint.exe", i 0x10000, i 7, p 0, i 3, i 0, p 0) p.r0'
    ${If} $0 == -1
      SetErrorLevel 23
      Quit
    ${EndIf}
    System::Call 'kernel32::CloseHandle(p r0)'
  unlocked:
FunctionEnd

Section "Install"
  InitPluginsDir
  SetOutPath "$PLUGINSDIR\payload"
  ClearErrors
  File /r "${CONTENT}\*"
  ${If} ${Errors}
    SetErrorLevel 22
    Quit
  ${EndIf}
  CreateDirectory "$INSTDIR"
  ; Refuse a locked/running program before changing any installed payload.
  StrCpy $HadPrevious 0
  IfFileExists "$INSTDIR\velo-slint.exe" 0 previous_done
    ClearErrors
    Rename "$INSTDIR\velo-slint.exe" "$INSTDIR\velo-slint.previous.exe"
    ${If} ${Errors}
      SetErrorLevel 23
      Quit
    ${EndIf}
    StrCpy $HadPrevious 1
  previous_done:
  !ifdef VALIDATION_INSTALL_DIR
    ${GetParameters} $Parameters
    ${GetOptions} $Parameters "/FAILAFTERBACKUP" $FailAfterBackup
    ${IfNot} ${Errors}
      Goto restore_previous
    ${EndIf}
  !endif
  ClearErrors
  CopyFiles /SILENT "$PLUGINSDIR\payload\*.*" "$INSTDIR"
  ${If} ${Errors}
    Goto restore_previous
  ${EndIf}
  IfFileExists "$INSTDIR\velo-slint.exe" 0 restore_previous
  WriteUninstaller "$INSTDIR\uninstall.exe"
  FileOpen $0 "$INSTDIR\preview-owner.txt" w
  FileWrite $0 "Velo Rust Preview ${VERSION} ${RENDERER}$\r$\n"
  FileClose $0
  !ifndef VALIDATION_INSTALL_DIR
    CreateShortcut "$SMPROGRAMS\Velo Rust Preview.lnk" "$INSTDIR\velo-slint.exe" '--instance-id=preview --data-dir="$LOCALAPPDATA\VeloSlintPreview"'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVeloSlintPreview" "DisplayName" "Velo Rust Preview (${RENDERER})"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVeloSlintPreview" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVeloSlintPreview" "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVeloSlintPreview" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVeloSlintPreview" "NoRepair" 1
  !endif
  Delete "$INSTDIR\velo-slint.previous.exe"
  SetErrorLevel 0
  Goto finished
  restore_previous:
    Delete "$INSTDIR\velo-slint.exe"
    ${If} $HadPrevious == 1
      Rename "$INSTDIR\velo-slint.previous.exe" "$INSTDIR\velo-slint.exe"
    ${EndIf}
    SetErrorLevel 24
  finished:
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  ; The generated list contains only packaged files. Unknown files and all
  ; external user data/backups survive uninstall; never RMDir /r $INSTDIR.
  !include "${UNINSTALL_FILES}"
  Delete "$INSTDIR\preview-owner.txt"
  Delete "$INSTDIR\uninstall.exe"
  !ifndef VALIDATION_INSTALL_DIR
    Delete "$SMPROGRAMS\Velo Rust Preview.lnk"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVeloSlintPreview"
  !endif
  RMDir "$INSTDIR"
SectionEnd
