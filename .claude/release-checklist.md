# Agent Pulse Release Checklist

## Version Numbers
- [x] Confirm version numbers are consistent and updated across `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`

## Build & Tests
- [x] Run `cd src-tauri && cargo test` — all tests pass
- [x] Run `npm run tauri build` — build completes without errors
- [x] No leftover debug/dev code (console.log, dbg!, hardcoded test values, etc.)

## Functionality
- [ ] System tray icon displays correctly, right-click menu works
- [ ] Webhook server starts and receives events (port 19280-19289)
- [ ] Claude provider hook installs/removes correctly
- [ ] OpenCode provider plugin installs/removes correctly
- [ ] Session state machine transitions work (Idle -> Working -> WaitingForUser)
- [ ] PID detection and automatic session removal work

## Cross-Platform
- [x] Windows build and test pass
- [ ] macOS build and test pass
- [ ] Linux build and test pass

## Release Prep
- [ ] README.md is up to date (features, setup instructions, screenshots, etc.)
- [ ] CHANGELOG or release notes updated
- [ ] Git tag created (format: v0.0.2)
