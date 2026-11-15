Unicode True
!include "MUI2.nsh"
!include "LogicLib.nsh"

!ifndef APP_EXE
  !error "Pass /DAPP_EXE=path-to-laborlens-ph.exe"
!endif
!ifndef OUTPUT_FILE
  !define OUTPUT_FILE "LaborLensPH-Setup-0.1.0.exe"
!endif
!ifndef NOTICES_FILE
  !error "Pass /DNOTICES_FILE=path-to-THIRD_PARTY_NOTICES.txt"
!endif

Name "LaborLens PH"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\LaborLens PH"
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "..\..\assets\icon.ico"
UninstallIcon "..\..\assets\icon.ico"
BrandingText "LaborLens PH - Desktop foundation"
VIProductVersion "0.1.0.0"
VIAddVersionKey /LANG=1033 "ProductName" "LaborLens PH"
VIAddVersionKey /LANG=1033 "FileDescription" "LaborLens PH desktop foundation installer"
VIAddVersionKey /LANG=1033 "FileVersion" "0.1.0"
VIAddVersionKey /LANG=1033 "LegalCopyright" "2026 LaborLens PH contributors"

!define MUI_WELCOMEPAGE_TITLE "Welcome to LaborLens PH"
!define MUI_WELCOMEPAGE_TEXT "Install the local desktop foundation for LaborLens PH.$\r$\n$\r$\nThis version provides a private document workspace and local preferences. Legal checking and automatic source updates are not available yet."
!define MUI_FINISHPAGE_RUN "$INSTDIR\LaborLensPH.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open LaborLens PH"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "LaborLens PH" SEC_APP
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /oname=LaborLensPH.exe "${APP_EXE}"
  File /oname=LaborLensPH.ico "..\..\assets\icon.ico"
  File /oname=LICENSE.txt "..\..\LICENSE"
  File /oname=Lato-OFL.txt "..\..\assets\fonts\OFL.txt"
  File /oname=THIRD_PARTY_NOTICES.txt "${NOTICES_FILE}"
  File /oname=Read-me.txt "..\..\packaging\windows\Read-me.txt"
  SetOutPath "$INSTDIR\Samples"
  File "..\..\examples\Synthetic sample.pdf"
  File "..\..\examples\Synthetic sample.docx"
  SetOutPath "$INSTDIR"
  CreateShortCut "$SMPROGRAMS\LaborLens PH.lnk" "$INSTDIR\LaborLensPH.exe" "" "$INSTDIR\LaborLensPH.ico"
  CreateShortCut "$DESKTOP\LaborLens PH.lnk" "$INSTDIR\LaborLensPH.exe" "" "$INSTDIR\LaborLensPH.ico"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "DisplayName" "LaborLens PH"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "DisplayVersion" "0.1.0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "Publisher" "LaborLens PH contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "DisplayIcon" "$INSTDIR\LaborLensPH.ico"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\LaborLens PH.lnk"
  Delete "$DESKTOP\LaborLens PH.lnk"
  Delete "$INSTDIR\LaborLensPH.exe"
  Delete "$INSTDIR\LaborLensPH.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\Lato-OFL.txt"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
  Delete "$INSTDIR\Read-me.txt"
  Delete "$INSTDIR\Samples\Synthetic sample.pdf"
  Delete "$INSTDIR\Samples\Synthetic sample.docx"
  RMDir "$INSTDIR\Samples"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\LaborLensPH"
  ; Deliberately preserve local workspace data and every original document.
SectionEnd
