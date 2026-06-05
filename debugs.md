# Debug Notes

## Duplicate `source` field breaks SessionStart detection (2026-03-26)

**Symptom**: New Claude Code sessions not detected at SessionStart — they only appear after the first UserPromptSubmit. Other events (PreToolUse, PostToolUse) work fine.

**Root cause**: Claude Code includes a `source` field in some hook events (SessionStart, SessionEnd) but not others (PreToolUse, PostToolUse). The hook script's `sed` was unconditionally appending `,"source":"claude"` to every event, creating a duplicate `source` field in SessionStart payloads. `serde_json` rejects duplicate fields by default, so the webhook returned 400 and the session was never created.

**Why it was hard to find**: The webhook's error (`Failed to parse hook event: duplicate field 'source'`) was only printed to stderr. The hook script's `curl` exit code was 0 (HTTP request succeeded), so the hook appeared to work. Only adding `cargo tauri dev` stderr logging revealed the parse error.

**Fix**: Check if `source` already exists in the JSON before injecting it:
```bash
if echo "$INPUT" | grep -q '"source"'; then
  # source already present (e.g. SessionStart) — only add pid
  echo "$INPUT" | sed "s/}$/,\"pid\":${CPID:-0}}/"
else
  # no source (e.g. tool use events) — add both pid and source
  echo "$INPUT" | sed "s/}$/,\"pid\":${CPID:-0},\"source\":\"claude\"}/"
fi
```

**Lesson**: When injecting fields into JSON via `sed`, never assume the upstream payload won't already contain that field. Claude Code can add new fields to hook events at any time.

## SessionEnd hook cancelled on Windows (2026-06-05)

**Symptom**: Claude Code prints `SessionEnd hook [bash "C:/Users/jagua/.claude/agent-pulse-hook.sh"] failed: Hook cancelled` on Windows when a session ends.

**Root cause**: SessionEnd fires while the agent is shutting down, so Claude Code only gives the hook a short window before cancelling it. On Windows the hook calls `find_claude_pid`, which spawns `powershell.exe` to walk the process tree (~2s). That overran the shutdown window, so Claude Code cancelled the hook before it finished. Harmless (the session still ends, and the 5s PID liveness check / 5-min timeout fallback still removes the session in the UI), but it surfaces a scary-looking error.

**Fix**: Skip the slow PID lookup for SessionEnd — the backend removes sessions by `session_id`, not PID (`session_manager.rs` SessionEnd arm), so the PID is never used there:
```bash
if echo "$INPUT" | grep -q '"hook_event_name"[[:space:]]*:[[:space:]]*"SessionEnd"'; then
  CPID=0
else
  CPID=$(find_claude_pid)
fi
```

**Lesson**: Lifecycle-end hooks run under tight time pressure. Don't do slow work (especially spawning `powershell.exe`) in a hook that fires during shutdown unless the result is actually needed for that event.
