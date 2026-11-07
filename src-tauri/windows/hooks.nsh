; Optional PATH entry, so remise-cli.exe (the command line, src/bin/remise-cli.rs) works in any
; terminal. Asked after install; `/PATH` says yes up front (silent installs, CI). Removed again on
; uninstall, kept across updates.
;
; PowerShell edits the user's Path in the registry: no NSIS string limit to truncate a long one,
; and %VARIABLES% in it stay unexpanded. Our folder travels in an environment variable, not inside
; the command, so a quote in the user name can't break it. Setting and clearing a dummy variable
; is what tells open programs (Explorer) that the environment changed.
!define REMISE_PATH_PS `powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$k = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment'); $$d = $$env:REMISE_DIR; $$p = @(([string]$$k.GetValue('Path', '', 'DoNotExpandEnvironmentNames')) -split ';' | Where-Object { $$_ -and $$_ -ne $$d }); if ($$env:REMISE_PATH_ADD) { $$p += $$d }; if ($$p) { $$k.SetValue('Path', ($$p -join ';'), 'ExpandString') } else { $$k.DeleteValue('Path', $$false) }; [Environment]::SetEnvironmentVariable('REMISE_PATH_REFRESH', '1', 'User'); [Environment]::SetEnvironmentVariable('REMISE_PATH_REFRESH', $$null, 'User')"`

!macro NSIS_HOOK_POSTINSTALL
  ${If} $UpdateMode <> 1
    ClearErrors
    ${GetOptions} $CMDLINE "/PATH" $R0
    ${If} ${Errors}
      ${If} ${Silent}
        Goto remise_path_done
      ${EndIf}
      MessageBox MB_YESNO|MB_ICONQUESTION "Use Remise from a terminal too?$\r$\n$\r$\nThis adds its folder to your PATH, so that remise-cli works in new terminal windows (try: remise-cli list)." IDNO remise_path_done
    ${EndIf}
    System::Call 'Kernel32::SetEnvironmentVariable(t "REMISE_DIR", t "$INSTDIR")'
    System::Call 'Kernel32::SetEnvironmentVariable(t "REMISE_PATH_ADD", t "1")'
    nsExec::Exec `${REMISE_PATH_PS}`
    Pop $0
  ${EndIf}
  remise_path_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    System::Call 'Kernel32::SetEnvironmentVariable(t "REMISE_DIR", t "$INSTDIR")'
    nsExec::Exec `${REMISE_PATH_PS}`
    Pop $0
  ${EndIf}
!macroend
