use super::HookProvider;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const HOOK_MARKER: &str = "127.0.0.1:192";

const HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "Stop",
];

fn settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

fn helper_script_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("agent-pulse-hook.sh"))
}

fn helper_script_content(port: u16, source: &str) -> String {
    format!(
        r#"#!/bin/bash
# Agent Pulse hook helper — injects PID and forwards to webhook server

# Walk up the process tree to find the claude process PID
find_claude_pid() {{
  if [ -f "/proc/$$/winpid" ]; then
    # Windows (Git Bash/MSYS): use PowerShell to walk process tree
    local MYWPID=$(cat /proc/$$/winpid)
    powershell.exe -NoProfile -Command "
      \$p = $MYWPID
      for (\$i = 0; \$i -lt 10; \$i++) {{
        \$proc = Get-CimInstance Win32_Process -Filter ('ProcessId='+\$p) -EA SilentlyContinue
        if (-not \$proc) {{ break }}
        if (\$proc.Name -match 'claude' -or \$proc.CommandLine -match 'claude-code') {{ \$proc.ProcessId; break }}
        \$p = \$proc.ParentProcessId
      }}
    " 2>/dev/null | tr -dc '0-9'
  elif [ -d "/proc/$PPID" ]; then
    # Linux: walk /proc to find claude ancestor
    local P=$PPID
    for _ in $(seq 1 10); do
      [ -d "/proc/$P" ] || break
      local NAME=$(cat /proc/$P/comm 2>/dev/null)
      local CMDL=$(tr '\0' ' ' < /proc/$P/cmdline 2>/dev/null)
      if echo "$NAME" | grep -qi claude; then echo "$P"; return; fi
      if echo "$CMDL" | grep -qi claude-code; then echo "$P"; return; fi
      P=$(awk '{{print $4}}' /proc/$P/stat 2>/dev/null)
      [ -z "$P" ] || [ "$P" = "0" ] || [ "$P" = "1" ] && break
    done
  else
    # macOS: walk process tree via ps
    local P=$PPID
    for _ in $(seq 1 10); do
      local NAME=$(ps -p "$P" -o comm= 2>/dev/null)
      [ -z "$NAME" ] && break
      if echo "$NAME" | grep -qi claude; then echo "$P"; return; fi
      local ARGS=$(ps -p "$P" -o args= 2>/dev/null)
      if echo "$ARGS" | grep -qi claude-code; then echo "$P"; return; fi
      P=$(ps -p "$P" -o ppid= 2>/dev/null | tr -dc '0-9')
      [ -z "$P" ] || [ "$P" = "0" ] || [ "$P" = "1" ] && break
    done
  fi
}}

# Capture stdin immediately before any slow operations
INPUT=$(cat)
CPID=$(find_claude_pid)
if echo "$INPUT" | grep -q '"source"'; then
  echo "$INPUT" | sed "s/}}$/,\"pid\":${{CPID:-0}}}}/"
else
  echo "$INPUT" | sed "s/}}$/,\"pid\":${{CPID:-0}},\"source\":\"{source}\"}}/"
fi | curl -s -X POST http://127.0.0.1:{port} -H "Content-Type: application/json" -d @-
"#
    )
}

fn make_hook_entry() -> Value {
    let script_path = helper_script_path()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    serde_json::json!({
        "matcher": "",
        "hooks": [
            {
                "type": "command",
                "command": format!("bash \"{}\"", script_path)
            }
        ]
    })
}

fn is_our_hook(entry: &Value) -> bool {
    if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
        hooks.iter().any(|hook| {
            hook.get("command")
                .and_then(|c| c.as_str())
                .map(|c| c.contains(HOOK_MARKER) || c.contains("agent-pulse-hook"))
                .unwrap_or(false)
        })
    } else {
        false
    }
}

pub struct ClaudeCodeProvider;

impl HookProvider for ClaudeCodeProvider {
    fn id(&self) -> &str {
        "claude"
    }

    fn display_name(&self) -> &str {
        "Claude Code"
    }

    fn badge_label(&self) -> &str {
        "CC"
    }

    fn badge_color(&self) -> &str {
        "#a78bfa"
    }

    fn install(&self, port: u16) -> Result<(), String> {
        let path = settings_path().ok_or("Could not determine home directory")?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create .claude dir: {}", e))?;
        }

        let script_path = helper_script_path().ok_or("Could not determine helper script path")?;
        fs::write(&script_path, helper_script_content(port, self.id()))
            .map_err(|e| format!("Failed to write hook helper script: {}", e))?;

        let mut settings: Value = if path.exists() {
            let content =
                fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {}", e))?;
            serde_json::from_str(&content).map_err(|e| {
                format!(
                    "Failed to parse settings.json: {}. Please fix it manually.",
                    e
                )
            })?
        } else {
            serde_json::json!({})
        };

        if settings.get("hooks").is_none() {
            settings["hooks"] = serde_json::json!({});
        }

        let hook_entry = make_hook_entry();

        for event_name in HOOK_EVENTS {
            let hooks_obj = settings["hooks"].as_object_mut().unwrap();

            if let Some(event_hooks) = hooks_obj.get_mut(*event_name) {
                if let Some(arr) = event_hooks.as_array_mut() {
                    arr.retain(|entry| !is_our_hook(entry));
                    arr.push(hook_entry.clone());
                }
            } else {
                hooks_obj.insert(event_name.to_string(), serde_json::json!([hook_entry]));
            }
        }

        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &content).map_err(|e| format!("Failed to write settings: {}", e))?;
        fs::rename(&tmp_path, &path).map_err(|e| format!("Failed to save settings: {}", e))?;

        println!("Claude Code integration installed for port {}", port);
        Ok(())
    }

    fn remove(&self) -> Result<(), String> {
        let path = settings_path().ok_or("Could not determine home directory")?;

        if let Some(script_path) = helper_script_path() {
            let _ = fs::remove_file(script_path);
        }

        if !path.exists() {
            return Ok(());
        }

        let content =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {}", e))?;
        let mut settings: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings.json: {}", e))?;

        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            for event_name in HOOK_EVENTS {
                if let Some(event_hooks) = hooks.get_mut(*event_name) {
                    if let Some(arr) = event_hooks.as_array_mut() {
                        arr.retain(|entry| !is_our_hook(entry));
                        if arr.is_empty() {
                            hooks.remove(*event_name);
                        }
                    }
                }
            }

            if hooks.is_empty() {
                settings.as_object_mut().unwrap().remove("hooks");
            }
        }

        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        fs::write(&path, content).map_err(|e| format!("Failed to write settings: {}", e))?;

        println!("Claude Code integration removed");
        Ok(())
    }

    fn is_installed(&self) -> bool {
        let path = match settings_path() {
            Some(p) => p,
            None => return false,
        };

        if !path.exists() {
            return false;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let settings: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return false,
        };

        if let Some(hooks) = settings.get("hooks").and_then(|h| h.as_object()) {
            hooks.values().any(|event_hooks| {
                event_hooks
                    .as_array()
                    .map(|arr| arr.iter().any(|entry| is_our_hook(entry)))
                    .unwrap_or(false)
            })
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_our_hook() {
        let hook = serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "bash \"/home/user/.claude/agent-pulse-hook.sh\""
            }]
        });
        assert!(is_our_hook(&hook));
    }

    #[test]
    fn test_is_our_hook_legacy() {
        let hook = serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "curl -s -X POST http://127.0.0.1:19280 -d @-"
            }]
        });
        assert!(is_our_hook(&hook));
    }

    #[test]
    fn test_is_not_our_hook() {
        let hook = serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "echo hello"
            }]
        });
        assert!(!is_our_hook(&hook));
    }

    #[test]
    fn test_make_hook_entry() {
        let entry = make_hook_entry();
        let cmd = entry["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("agent-pulse-hook"));
    }

    #[test]
    fn test_provider_metadata() {
        let p = ClaudeCodeProvider;
        assert_eq!(p.id(), "claude");
        assert_eq!(p.badge_label(), "CC");
    }
}
