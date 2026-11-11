; Raygun's uninstaller and its data (bundle > windows > nsis > installerHooks).
;
; Tauri's uninstaller asks whether to delete the app's data ("Delete the application data",
; unticked by default, so the data is kept) and, when ticked, removes %APPDATA%\app.raygun.player
; and %LOCALAPPDATA%\app.raygun.player: where Raygun keeps its data in installed mode. Raygun's usual
; place is the portable folder beside Raygun.exe, raygun-data (crates/raygun-paths), which the
; uninstaller leaves alone: it goes here too when the box is ticked. An update runs the uninstaller
; with /UPDATE and never deletes data.

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RMDir /r "$INSTDIR\raygun-data"
    RMDir "$INSTDIR"
  ${EndIf}
!macroend