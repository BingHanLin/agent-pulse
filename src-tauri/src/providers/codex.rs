use super::HookProvider;
use serde_json::Value;
use std::{fs, path::PathBuf};

const SH_FILE: &str = "agent-pulse-hook.sh";
const PS_FILE: &str = "agent-pulse-hook.ps1";
const EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Stop",
];

fn codex_dir() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
}
fn hooks_path() -> Option<PathBuf> {
    codex_dir().map(|d| d.join("hooks.json"))
}
fn sh_path() -> Option<PathBuf> {
    codex_dir().map(|d| d.join(SH_FILE))
}
fn ps_path() -> Option<PathBuf> {
    codex_dir().map(|d| d.join(PS_FILE))
}

fn sh_content(port: u16) -> String {
    format!(
        r#"#!/bin/sh
INPUT=$(cat)
P=$PPID; CPID=0; I=0
while [ "$P" -gt 1 ] 2>/dev/null && [ "$I" -lt 12 ]; do
  NAME=$(ps -p "$P" -o comm= 2>/dev/null); ARGS=$(ps -p "$P" -o args= 2>/dev/null)
  case "$NAME $ARGS" in *codex*|*Codex*) CPID=$P; break ;; esac
  P=$(ps -p "$P" -o ppid= 2>/dev/null | tr -dc '0-9'); I=$((I + 1))
done
PAYLOAD=$(printf '%s' "$INPUT" | sed 's/}}[[:space:]]*$/,"pid":'"${{CPID:-0}}"',"source":"codex"}}/')
curl -s -o /dev/null --max-time 2 -X POST "http://127.0.0.1:{port}" -H "Content-Type: application/json" -d "$PAYLOAD" 2>/dev/null || true
"#
    )
}

fn ps_content(port: u16) -> String {
    format!(
        r#"$ErrorActionPreference = "SilentlyContinue"
$payload = [Console]::In.ReadToEnd() | ConvertFrom-Json
$candidatePid = $PID; $codexPid = 0
for ($i = 0; $i -lt 12; $i++) {{
  $process = Get-CimInstance Win32_Process -Filter "ProcessId=$candidatePid"
  if (-not $process) {{ break }}
  if ($process.Name -match "codex" -or $process.CommandLine -match "codex") {{ $codexPid = $process.ProcessId; break }}
  $candidatePid = $process.ParentProcessId
}}
$payload | Add-Member -NotePropertyName "pid" -NotePropertyValue $codexPid -Force
$payload | Add-Member -NotePropertyName "source" -NotePropertyValue "codex" -Force
$json = $payload | ConvertTo-Json -Depth 20 -Compress
Invoke-WebRequest -UseBasicParsing -Method Post -Uri "http://127.0.0.1:{port}" -ContentType "application/json" -Body $json -TimeoutSec 2 | Out-Null
exit 0
"#
    )
}

fn make_hook_entry() -> Value {
    let sh = sh_path()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let ps = ps_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    serde_json::json!({"matcher":"", "hooks":[{
        "type":"command", "command":format!("sh \"{}\"", sh),
        "commandWindows":format!("powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\"", ps), "timeout":5
    }]})
}

fn is_ours(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .map(|hooks| {
            hooks.iter().any(|hook| {
                ["command", "commandWindows", "command_windows"]
                    .iter()
                    .filter_map(|key| hook.get(*key).and_then(Value::as_str))
                    .any(|command| command.contains(SH_FILE) || command.contains(PS_FILE))
            })
        })
        .unwrap_or(false)
}

pub struct CodexProvider;

impl HookProvider for CodexProvider {
    fn id(&self) -> &str {
        "codex"
    }
    fn display_name(&self) -> &str {
        "Codex"
    }
    fn badge_label(&self) -> &str {
        "CX"
    }
    fn badge_color(&self) -> &str {
        "#10a37f"
    }

    fn install(&self, port: u16) -> Result<(), String> {
        let dir = codex_dir().ok_or("Could not determine Codex home directory")?;
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create Codex directory: {e}"))?;
        fs::write(dir.join(SH_FILE), sh_content(port))
            .map_err(|e| format!("Failed to write shell hook: {e}"))?;
        fs::write(dir.join(PS_FILE), ps_content(port))
            .map_err(|e| format!("Failed to write PowerShell hook: {e}"))?;
        let path = hooks_path().ok_or("Could not determine Codex hooks path")?;
        let mut root: Value = if path.exists() {
            serde_json::from_str(
                &fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read hooks.json: {e}"))?,
            )
            .map_err(|e| format!("Failed to parse hooks.json: {e}"))?
        } else {
            serde_json::json!({})
        };
        if !root.is_object() {
            return Err("Codex hooks.json must contain an object".into());
        }
        if root.get("hooks").is_none() {
            root["hooks"] = serde_json::json!({});
        }
        let hooks = root["hooks"]
            .as_object_mut()
            .ok_or("Codex 'hooks' must be an object")?;
        let entry = make_hook_entry();
        for event in EVENTS {
            let entries = hooks
                .entry(*event)
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .ok_or_else(|| format!("Codex hook event '{event}' must be an array"))?;
            entries.retain(|item| !is_ours(item));
            entries.push(entry.clone());
        }
        let content = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| format!("Failed to save hooks: {e}"))
    }

    fn remove(&self) -> Result<(), String> {
        if let Some(p) = sh_path() {
            let _ = fs::remove_file(p);
        }
        if let Some(p) = ps_path() {
            let _ = fs::remove_file(p);
        }
        let path = hooks_path().ok_or("Could not determine Codex hooks path")?;
        if !path.exists() {
            return Ok(());
        }
        let mut root: Value =
            serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("Failed to parse hooks.json: {e}"))?;
        if let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) {
            for event in EVENTS {
                if let Some(entries) = hooks.get_mut(*event).and_then(Value::as_array_mut) {
                    entries.retain(|item| !is_ours(item));
                    if entries.is_empty() {
                        hooks.remove(*event);
                    }
                }
            }
            if hooks.is_empty() {
                root.as_object_mut().unwrap().remove("hooks");
            }
        }
        fs::write(
            path,
            serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("Failed to write hooks: {e}"))
    }

    fn is_installed(&self) -> bool {
        let Some(path) = hooks_path() else {
            return false;
        };
        let Ok(content) = fs::read_to_string(path) else {
            return false;
        };
        let Ok(root) = serde_json::from_str::<Value>(&content) else {
            return false;
        };
        root.get("hooks")
            .and_then(Value::as_object)
            .map(|hooks| {
                EVENTS.iter().all(|event| {
                    hooks
                        .get(*event)
                        .and_then(Value::as_array)
                        .map(|entries| entries.iter().any(is_ours))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hook_is_cross_platform() {
        let entry = make_hook_entry();
        assert!(entry["hooks"][0]["commandWindows"]
            .as_str()
            .unwrap()
            .contains(PS_FILE));
        assert!(is_ours(&entry));
    }
    #[test]
    fn scripts_forward_source() {
        assert!(sh_content(19280).contains("\"source\":\"codex\""));
        assert!(ps_content(19280).contains("19280"));
    }
    #[test]
    fn metadata() {
        assert_eq!(CodexProvider.id(), "codex");
    }
}
