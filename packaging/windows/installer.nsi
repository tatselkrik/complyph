Unicode True
!include "MUI2.nsh"
!include "LogicLib.nsh"

!ifndef APP_EXE
  !error "Pass /DAPP_EXE=path-to-complyph.exe"
!endif
!ifndef UPDATER_EXE
  !error "Pass /DUPDATER_EXE=path-to-complyph-updater.exe"
!endif
!ifndef OUTPUT_FILE
  !define OUTPUT_FILE "ComplyPH-Setup-0.10.0.exe"
!endif
!ifndef NOTICES_FILE
  !error "Pass /DNOTICES_FILE=path-to-THIRD_PARTY_NOTICES.txt"
!endif

!ifndef OCR_UNINSTALL
  !error "Pass /DOCR_UNINSTALL=path-to-runtime-file-list"
!endif

Name "ComplyPH"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\ComplyPH"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "InstallLocation"
Var LegacyInstallDir
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "..\..\assets\icon.ico"
UninstallIcon "..\..\assets\icon.ico"
BrandingText "ComplyPH - Local OCR and folder intake"
VIProductVersion "0.10.0.0"
VIAddVersionKey /LANG=1033 "ProductName" "ComplyPH"
VIAddVersionKey /LANG=1033 "FileDescription" "ComplyPH local legal source installer"
VIAddVersionKey /LANG=1033 "FileVersion" "0.10.0"
VIAddVersionKey /LANG=1033 "LegalCopyright" "2026 ComplyPH contributors"

!define MUI_WELCOMEPAGE_TITLE "Welcome to ComplyPH"
!define MUI_WELCOMEPAGE_TEXT "Install ComplyPH for private document review.$\r$\n$\r$\nInspect document text beside findings, jump to highlighted passages, and open exact supporting provisions offline. Import local legal references with citation URLs, dates and verification notes. Add reviewer notes, track follow-up statuses, and export PDF reports locally. Includes offline OCR for up to 20 PDF pages and optional local folder intake. Six focused labour checks, including annual paid service incentive leave, and 19 starter references. Windows workspace data uses ProgramData\ComplyPH; old data is copied safely and retained as backup. Approved official URLs can be checked daily while the app is open. Changed downloads are preserved for review."
!define MUI_FINISHPAGE_RUN "$INSTDIR\ComplyPH.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open ComplyPH"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ReadRegStr $LegacyInstallDir HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "InstallLocation"
  StrCmp $LegacyInstallDir "" done
  IfFileExists "$LegacyInstallDir\LaborLensPH.exe" 0 done
  StrCpy $INSTDIR $LegacyInstallDir
  FindWindow $0 "" "LaborLens PH"
  StrCmp $0 0 done
  IfSilent 0 +2
  Abort
  MessageBox MB_OK "Close the earlier app before upgrading to ComplyPH."
  Abort
  done:
FunctionEnd

Section "ComplyPH" SEC_APP
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  StrCmp $INSTDIR $LegacyInstallDir 0 not_legacy_upgrade
  ClearErrors
  Delete "$INSTDIR\LaborLensPH.exe"
  IfErrors 0 +2
  Abort "Close the earlier app and run the installer again."
  Delete "$INSTDIR\LaborLensPH.ico"
  Delete "$SMPROGRAMS\LaborLens PH.lnk"
  Delete "$DESKTOP\LaborLens PH.lnk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH"
  not_legacy_upgrade:
  File /oname=ComplyPH.exe "${APP_EXE}"
  File /oname=complyph-updater.exe "${UPDATER_EXE}"
  File /oname=ComplyPH.ico "..\..\assets\icon.ico"
  File /oname=LICENSE.txt "..\..\LICENSE"
  File /oname=Lato-OFL.txt "..\..\assets\fonts\OFL.txt"
  File /oname=THIRD_PARTY_NOTICES.txt "${NOTICES_FILE}"
  File /oname=Read-me.txt "..\..\packaging\windows\Read-me.txt"
  File /oname=starter-manifest.json "..\..\library\starter\manifest.json"
  File /oname=Library-notes.md "..\..\library\README.md"
  SetOutPath "$INSTDIR\OCR"
  File /r "..\..\target\ocr-runtime\*"
  SetOutPath "$INSTDIR\Samples"
  File "..\..\examples\*"
  SetOutPath "$INSTDIR"
  CreateShortCut "$SMPROGRAMS\ComplyPH.lnk" "$INSTDIR\ComplyPH.exe" "" "$INSTDIR\ComplyPH.ico"
  CreateShortCut "$DESKTOP\ComplyPH.lnk" "$INSTDIR\ComplyPH.exe" "" "$INSTDIR\ComplyPH.ico"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "DisplayName" "ComplyPH"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "DisplayVersion" "0.10.0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "Publisher" "ComplyPH contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "DisplayIcon" "$INSTDIR\ComplyPH.ico"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\ComplyPH.lnk"
  Delete "$DESKTOP\ComplyPH.lnk"
  Delete "$INSTDIR\ComplyPH.exe"
  Delete "$INSTDIR\complyph-updater.exe"
  Delete "$INSTDIR\ComplyPH.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\Lato-OFL.txt"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  Delete "$INSTDIR\Read-me.txt"
  Delete "$INSTDIR\starter-manifest.json"
  Delete "$INSTDIR\Library-notes.md"
  Delete "$INSTDIR\Samples\Synthetic sample.pdf"
  Delete "$INSTDIR\Samples\Synthetic sample.docx"
  Delete "$INSTDIR\Samples\Synthetic employment.pdf"
  Delete "$INSTDIR\Samples\Synthetic workplace policy.docx"
  Delete "$INSTDIR\Samples\Synthetic scan.pdf"
  Delete "$INSTDIR\Samples\Synthetic mixed pages.pdf"
  Delete "$INSTDIR\Samples\Synthetic protected.pdf"
  Delete "$INSTDIR\Samples\Synthetic damaged.pdf"
  Delete "$INSTDIR\Samples\Synthetic damaged.docx"
  Delete "$INSTDIR\Samples\Synthetic labour baseline.pdf"
  Delete "$INSTDIR\Samples\Synthetic labour issues.docx"
  Delete "$INSTDIR\Samples\Synthetic labour ambiguous.docx"
  Delete "$INSTDIR\Samples\Synthetic OCR policy.pdf"
  Delete "$INSTDIR\Samples\Synthetic SIL baseline.pdf"
  Delete "$INSTDIR\Samples\Synthetic SIL issues.docx"
  Delete "$INSTDIR\Samples\Synthetic SIL ambiguous.docx"
  Delete "$INSTDIR\Samples\Synthetic SIL excluded.docx"
  Delete "$INSTDIR\Samples\Synthetic SIL unknown service.docx"
  Delete "$INSTDIR\Samples\expected-sil.json"
  Delete "$INSTDIR\Samples\expected-ocr.json"
  Delete "$INSTDIR\Samples\expected-checks.json"
  Delete "$INSTDIR\Samples\expected-results.json"
  Delete "$INSTDIR\Samples\synthetic-source.json"
  Delete "$INSTDIR\Samples\README.md"
  RMDir "$INSTDIR\Samples"
  !include "${OCR_UNINSTALL}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ComplyPH"
  ; Deliberately preserve local workspace data and every original document.
SectionEnd
