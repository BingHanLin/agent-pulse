; NSIS hooks for AgentPulse installer

!macro NSIS_HOOK_PREUNINSTALL
  ; Run the app with --cleanup to remove all agent integrations
  ; (hook scripts, settings entries, plugins) before files are deleted
  nsExec::ExecToLog '"$INSTDIR\AgentPulse.exe" --cleanup'
!macroend
