# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Dev Commands

```bash
npm install                    # Install Node dependencies (first time)
npm run tauri dev              # Dev mode with hot-reload
npm run tauri build            # Production build
cd src-tauri && cargo test     # Run Rust unit tests
```

## Architecture

Agent Pulse is a Tauri 2 system tray app that monitors AI coding agent sessions in real-time. The frontend is vanilla JS (no framework).

**Data flow:**
```
Agent hook/plugin → HTTP POST → WebhookServer (19280-19289)
  → mpsc channel → SessionManager (state machine)
  → Tauri emit("sessions-changed") → Frontend render
```

**Backend (Rust, `src-tauri/src/`):**
- `lib.rs` — App setup: spawns webhook server, event processing loop, and 5-second staleness check loop
- `webhook_server.rs` — Raw TCP HTTP server, parses `HookEvent` JSON from POST body
- `session_manager.rs` — Session state machine (Idle/Working/WaitingForUser), PID-based removal, pin ordering
- `providers/mod.rs` — `HookProvider` trait + `ProviderRegistry`; each provider installs/removes agent hooks
- `providers/claude.rs` — Generates a bash hook script (`~/.claude/agent-pulse-hook.sh`) and patches `~/.claude/settings.json`
- `providers/opencode.rs` — Writes a JS plugin to `~/.config/opencode/plugins/`
- `process_monitor.rs` — Batch PID liveness check via `sysinfo` crate
- `commands.rs` — Tauri IPC command handlers

**Frontend (vanilla JS, `src/`):**
- `capsule.js` — All UI logic: session rendering, settings panel, drag-reorder, sound
- `styles.css` — Dark/light/OLED themes, CSS custom properties for state colors

## Key Design Decisions

- **Provider trait system**: Adding a new agent = one Rust file + one line in `create_registry()`. The frontend auto-discovers providers.
- **Hook script uses `sed` to inject fields into JSON**: The bash hook script appends `pid` (and conditionally `source`) to the JSON payload from the agent. See `debugs.md` for why `source` must be conditional.
- **PID detection**: The hook script walks the process tree upward from `$$` to find the agent process. On Windows this calls PowerShell (~2 seconds), so stdin is captured with `INPUT=$(cat)` before the PID lookup.
- **Session removal**: PID-based (preferred, checked every 5s) with 5-minute timeout fallback for sessions without a resolved PID.
- **Format strings in `providers/claude.rs`**: The hook script template uses Rust `format!()`, so all literal `{` `}` must be escaped as `{{` `}}`. Bash `${VAR}` becomes `${{VAR}}`, and JSON braces need similar doubling.

## Gotchas

- **`serde_json` rejects duplicate JSON fields** when deserializing into structs. The hook script must not inject fields that the agent already provides. See `debugs.md` for the full story.
- **Windows PID lookup**: `find_claude_pid` calls `powershell.exe` which is slow. The process name is `node.exe` (not `claude.exe`), so CommandLine matching (`claude-code`) is needed alongside Name matching.
- **Webhook server is raw TCP**: No HTTP library — it manually parses headers and handles partial reads via Content-Length.
