; ==============================================================================
; SAYL ("سيل") - NSIS Installer Hooks for Native Messaging Host & Extensions
; Runs in per-user scope (HKCU), never requiring administrative privileges.
; Zero-config browser extension flow for Chromium and Firefox.
; ==============================================================================

!include "LogicLib.nsh"

!macro NSIS_HOOK_PREINSTALL
  ; No pre-install action required
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Staging SAYL browser extensions to LocalAppData..."

  ; 1. Clean destination directories first to avoid stale files across upgrades
  RMDir /r "$LOCALAPPDATA\Sayl\extension\chromium"
  RMDir /r "$LOCALAPPDATA\Sayl\extension\firefox"
  CreateDirectory "$LOCALAPPDATA\Sayl\extension\chromium"
  CreateDirectory "$LOCALAPPDATA\Sayl\extension\firefox"

  ; 2. Stage unpacked Chromium and Firefox extension files via xcopy
  ${If} ${FileExists} "$INSTDIR\extensions\browser\chromium"
    nsExec::ExecToLog 'cmd.exe /c xcopy "$INSTDIR\extensions\browser\chromium" "$LOCALAPPDATA\Sayl\extension\chromium" /E /I /Y'
  ${EndIf}

  ${If} ${FileExists} "$INSTDIR\extensions\browser\firefox"
    nsExec::ExecToLog 'cmd.exe /c xcopy "$INSTDIR\extensions\browser\firefox" "$LOCALAPPDATA\Sayl\extension\firefox" /E /I /Y'
  ${EndIf}

  ; 3. Register native messaging host via sayl-bridge (single writer of manifests and HKCU keys)
  DetailPrint "Registering SAYL native messaging host..."
  ${If} ${FileExists} "$INSTDIR\host\sayl-bridge.exe"
    nsExec::ExecToLog '"$INSTDIR\host\sayl-bridge.exe" --register-host'
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Unregistering SAYL browser native messaging host..."

  ; 1. If sayl-bridge.exe exists, invoke --unregister-host
  ${If} ${FileExists} "$INSTDIR\host\sayl-bridge.exe"
    nsExec::ExecToLog '"$INSTDIR\host\sayl-bridge.exe" --unregister-host'
  ${EndIf}

  ; 2. Clean up installer-staged extension and host directories under %LOCALAPPDATA%\Sayl
  ; User downloads and settings are untouched.
  ; Note: The desktop app owns HKCU\Software\Microsoft\Windows\CurrentVersion\Run\SAYL,
  ; so the installer does not delete it on uninstall.
  RMDir /r "$LOCALAPPDATA\Sayl\extension"
  RMDir /r "$LOCALAPPDATA\Sayl\host"

  ; 3. Clean up host and extension directories inside install directory
  Delete "$INSTDIR\host\com.sayl.downloader.json"
  Delete "$INSTDIR\host\sayl-bridge.exe"
  Delete "$INSTDIR\host\icon.ico"
  RMDir "$INSTDIR\host"
  RMDir /r "$INSTDIR\extensions"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Additional safety cleanup of installer-staged directories
  RMDir /r "$LOCALAPPDATA\Sayl\extension"
  RMDir /r "$LOCALAPPDATA\Sayl\host"
!macroend
