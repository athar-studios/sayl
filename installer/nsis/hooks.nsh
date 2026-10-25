; ==============================================================================
; SAYL ("سيل") - NSIS Installer Hooks for Native Messaging Host Registration
; Runs in per-user scope (HKCU), never requiring administrative privileges.
; Pinned Extension ID: ipfenakpblanjjiboejfobkjhegnciif
; ==============================================================================

!include "LogicLib.nsh"
!include "WordFunc.nsh"

!macro NSIS_HOOK_PREINSTALL
  ; No pre-install action required
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Configuring SAYL browser native messaging host..."

  ; 1. If sayl-bridge.exe exists and supports --register-host, invoke it
  ${If} ${FileExists} "$INSTDIR\host\sayl-bridge.exe"
    nsExec::ExecToLog '"$INSTDIR\host\sayl-bridge.exe" --register-host --extension-id ipfenakpblanjjiboejfobkjhegnciif'
  ${EndIf}

  ; 2. Ensure the host manifest has the resolved absolute path and pinned extension ID
  ${If} ${FileExists} "$INSTDIR\host"
    ; Format $INSTDIR\host\sayl-bridge.exe with escaped backslashes for JSON
    ${WordReplace} "$INSTDIR\host\sayl-bridge.exe" "\" "\\" "+" $1

    ClearErrors
    FileOpen $0 "$INSTDIR\host\com.sayl.downloader.json" w
    ${Unless} ${Errors}
      FileWrite $0 "{\r$\n"
      FileWrite $0 '  "name": "com.sayl.downloader",$\r$\n'
      FileWrite $0 '  "description": "SAYL native messaging host",$\r$\n'
      FileWrite $0 '  "path": "$1",$\r$\n'
      FileWrite $0 '  "type": "stdio",$\r$\n'
      FileWrite $0 '  "allowed_origins": [$\r$\n'
      FileWrite $0 '    "chrome-extension://ipfenakpblanjjiboejfobkjhegnciif/"$\r$\n'
      FileWrite $0 "  ]$\r$\n"
      FileWrite $0 "}$\r$\n"
      FileClose $0
    ${EndUnless}
  ${EndIf}

  ; 3. Ensure HKCU NativeMessagingHosts registry keys are set for Chrome and Edge
  WriteRegStr HKCU "Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader" "" "$INSTDIR\host\com.sayl.downloader.json"
  WriteRegStr HKCU "Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader" "" "$INSTDIR\host\com.sayl.downloader.json"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Unregistering SAYL browser native messaging host..."

  ; 1. If sayl-bridge.exe exists, invoke --unregister-host
  ${If} ${FileExists} "$INSTDIR\host\sayl-bridge.exe"
    nsExec::ExecToLog '"$INSTDIR\host\sayl-bridge.exe" --unregister-host'
  ${EndIf}

  ; 2. Delete HKCU NativeMessagingHosts keys
  DeleteRegKey HKCU "Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader"
  DeleteRegKey HKCU "Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader"

  ; 3. Clean up autostart if user enabled run at startup
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "SAYL"

  ; 4. Clean up host manifest and installed extension directory
  Delete "$INSTDIR\host\com.sayl.downloader.json"
  Delete "$INSTDIR\host\sayl-bridge.exe"
  Delete "$INSTDIR\host\icon.ico"
  RMDir "$INSTDIR\host"
  RMDir /r "$INSTDIR\extensions"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Additional safety cleanup
  DeleteRegKey HKCU "Software\Google\Chrome\NativeMessagingHosts\com.sayl.downloader"
  DeleteRegKey HKCU "Software\Microsoft\Edge\NativeMessagingHosts\com.sayl.downloader"
!macroend
