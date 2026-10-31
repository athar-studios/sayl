; ==============================================================================
; SAYL ("سيل") - Modern NSIS Installer Script
; Per-user installation, no administrator privileges required.
; No telemetry, no background tracking, respects user privacy.
; ==============================================================================

Unicode true

; ------------------------------------------------------------------------------
; Version and Product Definitions
; ------------------------------------------------------------------------------
!define PRODUCT_NAME "SAYL"
!define PRODUCT_NAME_AR "سيل"
!define PRODUCT_PUBLISHER "SAYL Team"
!define PRODUCT_WEB_SITE "https://github.com"
!define MAIN_EXE "sayl.exe"

!ifndef VERSION
  ; Read version from installer/version.txt if not defined on CLI (-DVERSION=x.y.z)
  !searchparse /file "../version.txt" "" VERSION
!endif

!ifndef BUILD_DIR
  !define BUILD_DIR "..\..\apps\desktop\src-tauri\target\release"
!endif

Name "${PRODUCT_NAME} (${PRODUCT_NAME_AR}) ${VERSION}"
OutFile "..\..\releases\SAYL-Setup-x64-${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\SAYL"
InstallDirRegKey HKCU "Software\SAYL" "InstallDir"
RequestExecutionLevel user

; ------------------------------------------------------------------------------
; MUI2 Interface Configuration
; ------------------------------------------------------------------------------
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!include "WordFunc.nsh"

!define MUI_ABORTWARNING
; SAYL brand icon for the installer/uninstaller windows (paths are relative to
; this script, like OutFile and BUILD_DIR above).
!define MUI_ICON "..\..\apps\desktop\src-tauri\icons\icon.ico"
!define MUI_UNICON "..\..\apps\desktop\src-tauri\icons\icon.ico"

; Modern UI Header and Sidebar branding images
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "..\..\apps\desktop\src-tauri\icons\nsis-header.bmp"
!define MUI_HEADERIMAGE_UNBITMAP "..\..\apps\desktop\src-tauri\icons\nsis-header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "..\..\apps\desktop\src-tauri\icons\nsis-sidebar.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "..\..\apps\desktop\src-tauri\icons\nsis-sidebar.bmp"

; Installer Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${MAIN_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "تشغيل سيل الآن / Run SAYL now"
!insertmacro MUI_PAGE_FINISH

; Uninstaller Pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages (Arabic first, then English)
!insertmacro MUI_LANGUAGE "Arabic"
!insertmacro MUI_LANGUAGE "English"

; ------------------------------------------------------------------------------
; Installer Sections
; ------------------------------------------------------------------------------
Section "SAYL Core (مطلوب / Required)" SecCore
  SectionIn RO
  SetOutPath "$INSTDIR"
  
  ; Application binary
  File /nonfatal "${BUILD_DIR}\${MAIN_EXE}"
  
  ; Native messaging host and unpacked Chromium and Firefox extensions
  SetOutPath "$INSTDIR\host"
  File /nonfatal "${BUILD_DIR}\sayl-bridge.exe"
  
  SetOutPath "$INSTDIR\extensions\browser\chromium"
  File /r /x ".*" "..\..\extensions\browser\chromium\*.*"

  SetOutPath "$INSTDIR\extensions\browser\firefox"
  File /r /x ".*" "..\..\extensions\browser\firefox\*.*"

  ; Stage browser extensions under %LOCALAPPDATA%\Sayl\extension
  DetailPrint "Staging SAYL browser extensions to LocalAppData..."
  RMDir /r "$LOCALAPPDATA\Sayl\extension\chromium"
  RMDir /r "$LOCALAPPDATA\Sayl\extension\firefox"
  CreateDirectory "$LOCALAPPDATA\Sayl\extension\chromium"
  CreateDirectory "$LOCALAPPDATA\Sayl\extension\firefox"

  ${If} ${FileExists} "$INSTDIR\extensions\browser\chromium"
    nsExec::ExecToLog 'cmd.exe /c xcopy "$INSTDIR\extensions\browser\chromium" "$LOCALAPPDATA\Sayl\extension\chromium" /E /I /Y'
  ${EndIf}
  ${If} ${FileExists} "$INSTDIR\extensions\browser\firefox"
    nsExec::ExecToLog 'cmd.exe /c xcopy "$INSTDIR\extensions\browser\firefox" "$LOCALAPPDATA\Sayl\extension\firefox" /E /I /Y'
  ${EndIf}

  ; Register native messaging host via sayl-bridge (single writer of manifests and HKCU keys)
  DetailPrint "Registering SAYL native messaging host..."
  ${If} ${FileExists} "$INSTDIR\host\sayl-bridge.exe"
    nsExec::ExecToLog '"$INSTDIR\host\sayl-bridge.exe" --register-host'
  ${EndIf}

  ; Write uninstaller
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  
  ; Start Menu Shortcut
  CreateDirectory "$SMPROGRAMS\SAYL"
  CreateShortcut "$SMPROGRAMS\SAYL\SAYL.lnk" "$INSTDIR\${MAIN_EXE}" "" "$INSTDIR\${MAIN_EXE}" 0
  CreateShortcut "$SMPROGRAMS\SAYL\Uninstall.lnk" "$INSTDIR\Uninstall.exe" "" "$INSTDIR\Uninstall.exe" 0
  
  ; Registry entries for per-user Add/Remove Programs
  WriteRegStr HKCU "Software\SAYL" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\SAYL" "Version" "${VERSION}"
  
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "DisplayName" "SAYL - سيل"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "DisplayIcon" "$INSTDIR\${MAIN_EXE},0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "HelpLink" "${PRODUCT_WEB_SITE}"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL" "NoRepair" 1
SectionEnd

Section "اختصار سطح المكتب / Desktop Shortcut" SecDesktop
  CreateShortcut "$DESKTOP\SAYL.lnk" "$INSTDIR\${MAIN_EXE}" "" "$INSTDIR\${MAIN_EXE}" 0
SectionEnd

; Component descriptions
LangString DESC_SecCore ${LANG_ARABIC} "الملفات الأساسية لبرنامج سيل."
LangString DESC_SecCore ${LANG_ENGLISH} "Core SAYL application files."
LangString DESC_SecDesktop ${LANG_ARABIC} "إنشاء اختصار على سطح المكتب."
LangString DESC_SecDesktop ${LANG_ENGLISH} "Create a shortcut on the desktop."

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} $(DESC_SecCore)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} $(DESC_SecDesktop)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; ------------------------------------------------------------------------------
; Uninstaller Section
; ------------------------------------------------------------------------------
Section "Uninstall"
  ; Prompt user interactively (unless silent mode /S is active)
  IfSilent skip_prompt
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "هل ترغب في الاحتفاظ بملفات التنزيل والإعدادات الخاصة بك؟$\n$\nDo you want to keep your downloaded files and settings?" \
      /SD IDYES IDYES skip_data_cleanup IDNO cleanup_settings

  cleanup_settings:
    ; Clean only application config, NEVER user download directories
    RMDir /r "$APPDATA\com.sayl.downloadmanager"
    RMDir /r "$LOCALAPPDATA\com.sayl.downloadmanager"
    Goto done_cleanup

  skip_data_cleanup:
  skip_prompt:
  done_cleanup:

  ; Remove shortcuts
  Delete "$DESKTOP\SAYL.lnk"
  Delete "$SMPROGRAMS\SAYL\SAYL.lnk"
  Delete "$SMPROGRAMS\SAYL\Uninstall.lnk"
  RMDir "$SMPROGRAMS\SAYL"
  
  ; Native messaging unregistration
  ${If} ${FileExists} "$INSTDIR\host\sayl-bridge.exe"
    nsExec::ExecToLog '"$INSTDIR\host\sayl-bridge.exe" --unregister-host'
  ${EndIf}

  ; Clean up installer-staged extension and host directories under %LOCALAPPDATA%\Sayl
  ; User downloads and settings are untouched.
  ; Note: The desktop app owns HKCU\Software\Microsoft\Windows\CurrentVersion\Run\SAYL,
  ; so the installer does not delete it on uninstall.
  RMDir /r "$LOCALAPPDATA\Sayl\extension"
  RMDir /r "$LOCALAPPDATA\Sayl\host"

  ; Remove only installed application files
  Delete "$INSTDIR\${MAIN_EXE}"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$INSTDIR\host\com.sayl.downloader.json"
  Delete "$INSTDIR\host\sayl-bridge.exe"
  RMDir "$INSTDIR\host"
  RMDir /r "$INSTDIR\extensions"
  
  ; Remove install directory only if empty
  RMDir "$INSTDIR"
  
  ; Remove Add/Remove Programs registry keys
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL"
  DeleteRegKey HKCU "Software\SAYL"
SectionEnd
