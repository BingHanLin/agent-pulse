# Agent Pulse Release Checklist

## Version Numbers
- [x] Confirm version numbers are consistent and updated across `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and `Cargo.lock` (all at 0.0.4)

## Build & Tests
- [x] Run `cd src-tauri && cargo test` — all tests pass (18 passed)
- [x] `cargo fmt --check` clean and `cargo clippy -- -D warnings` clean (matches PR CI)
- [ ] Run `npm run tauri build` — build completes without errors (verified via CI release run on tag)
- [x] No leftover debug/dev code (console.log, dbg!, hardcoded test values, etc.)

## Functionality
- [ ] System tray icon displays correctly, right-click menu works
- [ ] Webhook server starts and receives events (port 19280-19289)
- [ ] Claude provider hook installs/removes correctly
- [ ] OpenCode provider plugin installs/removes correctly
- [ ] Session state machine transitions work (Idle -> Working -> WaitingForUser)
- [ ] PID detection and automatic session removal work
- [ ] Hide-to-tray no longer reappears via topmost re-assert (this release's fix)

## Cross-Platform
- [x] Windows build and test pass (local)
- [ ] macOS build and test pass (verify via CI release run)
- [ ] Linux build and test pass (verify via CI release run)

## Release Prep
- [ ] README.md is up to date (features, setup instructions, screenshots, etc.)
- [ ] CHANGELOG or release notes updated
- [ ] PR merged to main
- [ ] Git tag created (format: v0.0.4)
- [ ] Publish the draft GitHub Release after CI completes
