---
name: release-checklist
description: "Manage the Agent Pulse pre-release checklist. Use this skill when the user mentions release checklist, pre-release checks, release prep, release preparation, version release, build checks, or anything related to preparing a new version for release. Even if the user just asks 'what to do before release' or 'prepare for release', this skill should trigger."
---

# Release Checklist

This skill manages the pre-release checklist for Agent Pulse (a Tauri 2 system tray app). It tracks all verification steps that should be completed before each release.

## Workflow

### 1. Read or create the checklist

Check if `.claude/release-checklist.md` exists:

- **Exists** — Read the file and display the current status of each item (done / not done)
- **Does not exist** — Create it using the default template below

### 2. Default checklist template

When creating a new checklist, use this template. Each item is marked with `- [ ]` (incomplete) or `- [x]` (complete):

```markdown
# Agent Pulse Release Checklist

## Version Numbers
- [ ] Confirm version numbers are consistent and updated across `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`

## Build & Tests
- [ ] Run `cd src-tauri && cargo test` — all tests pass
- [ ] Run `npm run tauri build` — build completes without errors
- [ ] No leftover debug/dev code (console.log, dbg!, hardcoded test values, etc.)

## Functionality
- [ ] System tray icon displays correctly, right-click menu works
- [ ] Webhook server starts and receives events (port 19280-19289)
- [ ] Claude provider hook installs/removes correctly
- [ ] OpenCode provider plugin installs/removes correctly
- [ ] Session state machine transitions work (Idle -> Working -> WaitingForUser)
- [ ] PID detection and automatic session removal work

## Cross-Platform
- [ ] Windows build and test pass
- [ ] macOS build and test pass
- [ ] Linux build and test pass

## Release Prep
- [ ] README.md is up to date (features, setup instructions, screenshots, etc.)
- [ ] CHANGELOG or release notes updated
- [ ] Git tag created (format: v{version})
```

### 3. User interactions

The user can ask to:

- **View checklist** — Display all items and their status
- **Check off items** — Mark specific items as complete `[x]`
- **Uncheck items** — Mark items back to incomplete `[ ]`
- **Add items** — Add new check items under the appropriate section
- **Remove items** — Delete unneeded check items
- **Reset checklist** — Set all items back to incomplete (for a new release cycle)
- **Run checks** — Automatically verify items that can be automated (e.g., run tests, compare version numbers)

### 4. Automated verification

For these items, run commands directly to verify rather than relying on manual confirmation:

- **Version consistency** — Read all three files and compare version numbers
- **cargo test** — Run tests and report results
- **npm run tauri build** — Run the build (ask user for confirmation first since it takes a while)
- **Leftover debug code** — Search for `console.log`, `dbg!`, `todo!`, `FIXME`, etc.

When automated verification passes, check off the item and inform the user of the result.

### 5. Save

After each operation, write the updated checklist back to `.claude/release-checklist.md`.
