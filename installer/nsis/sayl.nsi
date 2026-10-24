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

!define MUI_ABORTWARNING
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\modern-uninstall.ico"

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
  
  ; Remove only installed application files
  Delete "$INSTDIR\${MAIN_EXE}"
  Delete "$INSTDIR\Uninstall.exe"
  
  ; Remove install directory only if empty
  RMDir "$INSTDIR"
  
  ; Remove Add/Remove Programs registry keys
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\SAYL"
  DeleteRegKey HKCU "Software\SAYL"
SectionEnd
