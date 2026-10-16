; Offline setup never downloads platform components. Registry checks mirror
; Tauri's WebView2 detection; this is a prerequisite, not a runtime health probe.
!macro NSIS_HOOK_PREINSTALL
  Push $R9
  ReadRegStr $R9 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${If} $R9 == ""
    ReadRegStr $R9 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${EndIf}
  ${If} $R9 == ""
    ReadRegStr $R9 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${EndIf}
  ${If} $R9 == ""
  ${OrIf} $R9 == "0.0.0.0"
    DetailPrint "Offline prerequisite missing: Microsoft Edge WebView2 Runtime. No download was attempted."
    IfSilent +2
      MessageBox MB_OK|MB_ICONSTOP "Custodian requires the Microsoft Edge WebView2 Runtime to be installed first. This offline installer will not download it. Install the runtime separately from a trusted offline installer, then run setup again."
    Pop $R9
    SetErrorLevel 2
    Abort
  ${EndIf}
  Pop $R9
!macroend
