; The command line (src/bin/remise-cli.rs) is installed as bin\remise.exe: next to Remise.exe, a
; remise.exe would be the same file, Windows ignores case. bin\ alone goes on the PATH, so `remise`
; in a terminal is the command line, as on Mac and Linux. Asked after install; `/PATH` says yes up
; front (silent installs, CI). Removed again on uninstall, kept across updates.
;
; PowerShell edits the user's Path in the registry: no NSIS string limit to truncate a long one,
; and %VARIABLES% in it stay unexpanded. Our folder travels in an environment variable, not inside
; the command, so a quote in the user name can't break it. Setting and clearing a dummy variable
; is what tells open programs (Explorer) that the environment changed.
!define REMISE_PATH_PS `powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$k = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment'); $$d = $$env:REMISE_DIR; $$p = @(([string]$$k.GetValue('Path', '', 'DoNotExpandEnvironmentNames')) -split ';' | Where-Object { $$_ -and $$_ -ne $$d }); if ($$env:REMISE_PATH_ADD) { $$p += $$d }; if ($$p) { $$k.SetValue('Path', ($$p -join ';'), 'ExpandString') } else { $$k.DeleteValue('Path', $$false) }; [Environment]::SetEnvironmentVariable('REMISE_PATH_REFRESH', '1', 'User'); [Environment]::SetEnvironmentVariable('REMISE_PATH_REFRESH', $$null, 'User')"`

!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$INSTDIR\bin"
  Delete "$INSTDIR\bin\remise.exe"
  Rename "$INSTDIR\remise-cli.exe" "$INSTDIR\bin\remise.exe"
  ${If} $UpdateMode <> 1
    ClearErrors
    ${GetOptions} $CMDLINE "/PATH" $R0
    ${If} ${Errors}
      ${If} ${Silent}
        Goto remise_path_done
      ${EndIf}
      MessageBox MB_YESNO|MB_ICONQUESTION "Use Remise from a terminal too?$\r$\n$\r$\nThis adds the remise command to your PATH, for new terminal windows (try: remise list)." IDNO remise_path_done
    ${EndIf}
    System::Call 'Kernel32::SetEnvironmentVariable(t "REMISE_DIR", t "$INSTDIR\bin")'
    System::Call 'Kernel32::SetEnvironmentVariable(t "REMISE_PATH_ADD", t "1")'
    nsExec::Exec `${REMISE_PATH_PS}`
    Pop $0
  ${EndIf}
  remise_path_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$INSTDIR\bin\remise.exe"
  RMDir "$INSTDIR\bin"
  ${If} $UpdateMode <> 1
    System::Call 'Kernel32::SetEnvironmentVariable(t "REMISE_DIR", t "$INSTDIR\bin")'
    nsExec::Exec `${REMISE_PATH_PS}`
    Pop $0
  ${EndIf}
!macroend
