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

fn make_hook_entry(port: u16) -> Value {
    serde_json::json!({
        "matcher": "",
        "hooks": [
            {
                "type": "command",
                "command": format!(
                    "curl -s -X POST http://127.0.0.1:{} -H \"Content-Type: application/json\" -d @-",
                    port
                )
            }
        ]
    })
}

fn is_our_hook(entry: &Value) -> bool {
    if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
        hooks.iter().any(|hook| {
            hook.get("command")
                .and_then(|c| c.as_str())
                .map(|c| c.contains(HOOK_MARKER))
                .unwrap_or(false)
        })
    } else {
        false
    }
}

pub fn install_hooks(port: u16) -> Result<(), String> {
    let path = settings_path().ok_or("Could not determine home directory")?;

    // Ensure .claude directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create .claude dir: {}", e))?;
    }

    // Read existing settings or start fresh
    let mut settings: Value = if path.exists() {
        let content =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings.json: {}. Please fix it manually.", e))?
    } else {
        serde_json::json!({})
    };

    // Ensure hooks object exists
    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({});
    }

    let hook_entry = make_hook_entry(port);

    for event_name in HOOK_EVENTS {
        let hooks_obj = settings["hooks"].as_object_mut().unwrap();

        if let Some(event_hooks) = hooks_obj.get_mut(*event_name) {
            if let Some(arr) = event_hooks.as_array_mut() {
                // Remove any existing ClaudePulse hooks (idempotent)
                arr.retain(|entry| !is_our_hook(entry));
                // Add our hook
                arr.push(hook_entry.clone());
            }
        } else {
            // Create new array with our hook
            hooks_obj.insert(event_name.to_string(), serde_json::json!([hook_entry]));
        }
    }

    // Write atomically (write to temp, rename)
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;
    fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    println!("Hooks installed for port {}", port);
    Ok(())
}

pub fn remove_hooks() -> Result<(), String> {
    let path = settings_path().ok_or("Could not determine home directory")?;

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
                    // Remove empty arrays
                    if arr.is_empty() {
                        hooks.remove(*event_name);
                    }
                }
            }
        }

        // Remove empty hooks object
        if hooks.is_empty() {
            settings.as_object_mut().unwrap().remove("hooks");
        }
    }

    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("Failed to write settings: {}", e))?;

    println!("Hooks removed");
    Ok(())
}

pub fn is_hooks_installed() -> bool {
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
        // Check if at least one event has our hook
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_our_hook() {
        let hook = serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "curl -s -X POST http://127.0.0.1:19280 -H \"Content-Type: application/json\" -d @-"
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
        let entry = make_hook_entry(19280);
        let cmd = entry["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("127.0.0.1:19280"));
        assert!(cmd.contains("curl"));
    }
}
