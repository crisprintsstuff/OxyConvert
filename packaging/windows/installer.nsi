; NSIS script for OxyConvert Windows Setup Installer
Unicode true
!define APPNAME "OxyConvert"
!define COMPANYNAME "OxyLabs"
!define DESCRIPTION "Universal Addon & Plugin Converter for OxyBrush"

!ifndef AppVersion
  !define AppVersion "0.1.1"
!endif

!ifndef SourceExe
  !define SourceExe "..\..\target\x86_64-pc-windows-msvc\release\oxyconvert.exe"
!endif

!ifndef OutSetupExe
  !define OutSetupExe "..\..\dist\OxyConvert-${AppVersion}-windows-x86_64-setup.exe"
!endif

!define HELPURL "https://github.com/crisprintsstuff/OxyConvert"
!define UPDATEURL "https://github.com/crisprintsstuff/OxyConvert/releases"
!define ABOUTURL "https://github.com/crisprintsstuff/OxyConvert"
!define INSTALLSIZE 18000

RequestExecutionLevel user
SetCompressor /SOLID lzma

InstallDir "$LOCALAPPDATA\Programs\OxyConvert"
InstallDirRegKey HKCU "Software\OxyLabs\OxyConvert" "InstallLocation"

Name "${APPNAME} ${AppVersion}"
OutFile "${OutSetupExe}"

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "..\..\packaging\icons\oxybrush.ico"
!define MUI_UNICON "..\..\packaging\icons\oxybrush.ico"

; Installer Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\oxyconvert.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch OxyConvert"
!insertmacro MUI_PAGE_FINISH

; Uninstaller Pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Install"
    SetOutPath "$INSTDIR"
    File /oname=oxyconvert.exe "${SourceExe}"
    File "..\..\README.md"
    File "..\..\SPEC_OXYADDON.md"
    File "..\..\LICENSE-MIT"
    File "..\..\LICENSE-APACHE"

    ; Uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; Start Menu Shortcuts
    CreateDirectory "$SMPROGRAMS\OxyConvert"
    CreateShortcut "$SMPROGRAMS\OxyConvert\OxyConvert.lnk" "$INSTDIR\oxyconvert.exe" "" "$INSTDIR\oxyconvert.exe" 0
    CreateShortcut "$SMPROGRAMS\OxyConvert\Uninstall OxyConvert.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0
    CreateShortcut "$DESKTOP\OxyConvert.lnk" "$INSTDIR\oxyconvert.exe" "" "$INSTDIR\oxyconvert.exe" 0

    ; Add/Remove Programs Registry Entries
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "DisplayName" "${APPNAME} - ${DESCRIPTION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "InstallLocation" "$\"$INSTDIR$\""
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "DisplayIcon" "$\"$INSTDIR\oxyconvert.exe$\""
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "Publisher" "${COMPANYNAME}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "HelpLink" "${HELPURL}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "URLUpdateInfo" "${UPDATEURL}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "URLInfoAbout" "${ABOUTURL}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "DisplayVersion" "${AppVersion}"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "NoRepair" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert" "EstimatedSize" ${INSTALLSIZE}

    ; File Association: .oxyaddon
    WriteRegStr HKCU "Software\Classes\.oxyaddon" "" "OxyConvert.Package"
    WriteRegStr HKCU "Software\Classes\OxyConvert.Package" "" "OxyBrush Addon Package"
    WriteRegStr HKCU "Software\Classes\OxyConvert.Package\DefaultIcon" "" "$INSTDIR\oxyconvert.exe,0"
    WriteRegStr HKCU "Software\Classes\OxyConvert.Package\shell\open\command" "" "$\"$INSTDIR\oxyconvert.exe$\" inspect $\"%1$\""
    WriteRegStr HKCU "Software\Classes\OxyConvert.Package\shell\Install to OxyBrush\command" "" "$\"$INSTDIR\oxyconvert.exe$\" install $\"%1$\""
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\oxyconvert.exe"
    Delete "$INSTDIR\README.md"
    Delete "$INSTDIR\SPEC_OXYADDON.md"
    Delete "$INSTDIR\LICENSE-MIT"
    Delete "$INSTDIR\LICENSE-APACHE"
    Delete "$INSTDIR\uninstall.exe"

    Delete "$SMPROGRAMS\OxyConvert\OxyConvert.lnk"
    Delete "$SMPROGRAMS\OxyConvert\Uninstall OxyConvert.lnk"
    RMDir "$SMPROGRAMS\OxyConvert"
    Delete "$DESKTOP\OxyConvert.lnk"

    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\OxyConvert"
    DeleteRegKey HKCU "Software\OxyLabs\OxyConvert"
    DeleteRegKey HKCU "Software\Classes\.oxyaddon"
    DeleteRegKey HKCU "Software\Classes\OxyConvert.Package"

    RMDir "$INSTDIR"
SectionEnd
