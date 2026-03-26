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
