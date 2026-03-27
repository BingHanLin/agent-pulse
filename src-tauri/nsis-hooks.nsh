; NSIS hooks for AgentPulse installer

!macro NSIS_HOOK_PREUNINSTALL
  ; Run the app with --cleanup to remove all agent integrations
  ; (hook scripts, settings entries, plugins) before files are deleted.
  ; Must use ExecWait (not nsExec) because the release binary has
  ; windows_subsystem = "windows" (GUI), and nsExec only works with
  ; console-subsystem executables.
  ExecWait '"$INSTDIR\AgentPulse.exe" --cleanup'
!macroend
