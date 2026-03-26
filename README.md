# Agent Pulse

A lightweight system tray app that monitors AI coding agent sessions in real-time. Built with [Tauri 2](https://tauri.app/).

![status](https://img.shields.io/badge/status-alpha-orange)

## Supported Agents

| Agent | Integration Method | Status |
|-------|-------------------|--------|
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | Hook scripts in `~/.claude/settings.json` | Supported |
| [OpenCode](https://opencode.ai) | JS plugin in `~/.config/opencode/plugins/` | Supported |

Adding a new agent requires only implementing the `HookProvider` trait in a single Rust file. See [Adding a New Provider](#adding-a-new-provider).

## Features

- Floating capsule widget showing active sessions and their states (Working, Waiting, Idle)
- Per-session metrics: elapsed time, last prompt, last tool used, project name, PID
- Configurable status colors, text size, and light/dark theme
- Sound notification on task completion
- Source badges (CC, OC, ...) to distinguish agent origins
- Auto-installs hooks/plugins on first launch

## How It Works

```
Agent (Claude Code / OpenCode / ...)
  → Hook/plugin fires on session events
  → HTTP POST to localhost:19280-19289
  → Agent Pulse webhook server receives HookEvent
  → SessionManager updates state machine
  → Frontend renders session list
```

All agents share the same webhook server and session manager. Each agent only needs a provider that knows how to install/remove its hook and translate events into the standard `HookEvent` format.

## Architecture

```
src-tauri/src/
├── lib.rs                        # App setup, event loop, provider registry init
├── main.rs                       # Entry point
├── providers/
│   ├── mod.rs                    # HookProvider trait, ProviderRegistry
│   ├── claude.rs                 # Claude Code provider (bash hook → settings.json)
│   └── opencode.rs               # OpenCode provider (JS plugin)
├── commands.rs                   # Tauri IPC commands (generic provider API)
├── session_manager.rs            # Session state machine (source-agnostic)
├── webhook_server.rs             # HTTP POST listener for hook events
├── process_monitor.rs            # PID liveness checks via sysinfo
├── settings.rs                   # User preferences (colors, theme, sound)
└── tray.rs                       # System tray menu

src/
├── index.html                    # UI layout
├── capsule.js                    # Frontend logic (vanilla JS, dynamic provider UI)
└── styles.css                    # Design system (dark/light themes)
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (for Tauri CLI)
- Platform-specific Tauri dependencies: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

### Run

```bash
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

### Test

```bash
cd src-tauri && cargo test
```

## Adding a New Provider

1. Create `src-tauri/src/providers/myagent.rs`:

```rust
use super::HookProvider;

pub struct MyAgentProvider;

impl HookProvider for MyAgentProvider {
    fn id(&self) -> &str { "myagent" }
    fn display_name(&self) -> &str { "My Agent" }
    fn badge_label(&self) -> &str { "MA" }
    fn badge_color(&self) -> &str { "#60a5fa" }

    fn install(&self, port: u16) -> Result<(), String> {
        // Write hook/plugin config that POSTs HookEvents to 127.0.0.1:{port}
        todo!()
    }

    fn remove(&self) -> Result<(), String> {
        // Remove the hook/plugin config
        todo!()
    }

    fn is_installed(&self) -> bool {
        // Check if the hook/plugin config exists
        todo!()
    }
}
```

2. Register it in `src-tauri/src/providers/mod.rs`:

```rust
pub mod myagent;

// In create_registry():
registry.register(Box::new(myagent::MyAgentProvider));
```

That's it. The frontend auto-discovers providers via `get_providers()` and renders them dynamically.

### HookEvent Format

Your agent's hook/plugin must POST JSON to `http://127.0.0.1:{port}`:

```json
{
  "session_id": "unique-session-id",
  "hook_event_name": "SessionStart",
  "cwd": "/path/to/project",
  "pid": 12345,
  "source": "myagent"
}
```

Supported `hook_event_name` values:
- `SessionStart` / `SessionEnd` — session lifecycle
- `UserPromptSubmit` — user sent a prompt (sets Working state)
- `PreToolUse` / `PostToolUse` — tool execution (include `tool_name`)
- `PermissionRequest` — waiting for user approval (sets WaitingForUser state)
- `Stop` — agent finished responding (sets Idle state)

## License

MIT
