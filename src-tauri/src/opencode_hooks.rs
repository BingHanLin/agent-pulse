use std::fs;
use std::path::PathBuf;

const PLUGIN_FILENAME: &str = "claude-pulse-opencode.js";

fn plugin_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".config").join("opencode").join("plugins"))
}

fn plugin_path() -> Option<PathBuf> {
    plugin_dir().map(|d| d.join(PLUGIN_FILENAME))
}

fn plugin_content(port: u16) -> String {
    format!(
        r#"// Claude Pulse — OpenCode plugin
// Auto-generated. Do not edit manually.
//
// OpenCode event structure notes:
//   - Events are nested: the handler receives {{ event: {{ type, properties }} }}
//   - session.status "busy"/"idle" maps to Working/Idle states
//   - question.asked maps to WaitingForUser (not "permission.asked")
//   - Tool use is via message.part.updated with part.type === "tool",
//     not "tool.execute.before/after"
//   - message.updated with role "user" fires spuriously during idle
//     (summary/diff updates), so we rely on session.status "busy" instead
//   - The "question" tool's message.part.updated events are skipped
//     because question.asked already handles WaitingForUser state
//
// Debounce trick:
//   OpenCode emits session.status "busy" ~35ms before "idle" at task
//   completion. Without debouncing, the late "busy" POST can arrive
//   after "idle" and flip the state back to Working. We use a 100ms
//   setTimeout on "busy" so that a following "idle" within that window
//   cancels the pending UserPromptSubmit via clearTimeout.

const PORT = {port};
const URL = `http://127.0.0.1:${{PORT}}`;

export const ClaudePulse = async ({{ directory }}) => {{
  const sessionId = `opencode-${{Date.now()}}-${{Math.random().toString(36).slice(2, 8)}}`;
  const cwd = directory || process.cwd();
  const pid = process.pid;

  const send = async (hookEventName, extra = {{}}) => {{
    try {{
      await fetch(URL, {{
        method: "POST",
        headers: {{ "Content-Type": "application/json" }},
        body: JSON.stringify({{
          session_id: sessionId,
          hook_event_name: hookEventName,
          cwd,
          pid,
          source: "opencode",
          ...extra,
        }}),
      }});
    }} catch {{}}
  }};

  await send("SessionStart");

  let busyTimer = null;

  return {{
    event: async (event) => {{
      const e = event?.event;
      if (!e) return;
      const t = e.type;
      const p = e.properties || {{}};

      switch (t) {{
        case "session.status":
          if (p.status?.type === "busy") {{
            clearTimeout(busyTimer);
            busyTimer = setTimeout(() => send("UserPromptSubmit"), 100);
          }} else if (p.status?.type === "idle") {{
            clearTimeout(busyTimer);
            await send("Stop");
          }}
          break;
        case "session.idle":
          clearTimeout(busyTimer);
          await send("Stop");
          break;
        case "session.deleted":
          await send("SessionEnd");
          break;
        case "question.asked":
          await send("PermissionRequest");
          break;
        case "message.part.updated": {{
          const part = p.part;
          if (part?.type === "tool" && part.tool !== "question") {{
            if (part.state?.status === "pending" || part.state?.status === "running") {{
              await send("PreToolUse", {{ tool_name: part.tool }});
            }} else if (part.state?.status === "completed" || part.state?.status === "error") {{
              await send("PostToolUse", {{ tool_name: part.tool }});
            }}
          }}
          break;
        }}
      }}
    }},
  }};
}};
"#
    )
}

pub fn install_opencode_plugin(port: u16) -> Result<(), String> {
    let dir = plugin_dir().ok_or("Could not determine config directory")?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create OpenCode plugins dir: {}", e))?;

    let path = dir.join(PLUGIN_FILENAME);
    fs::write(&path, plugin_content(port))
        .map_err(|e| format!("Failed to write OpenCode plugin: {}", e))?;

    println!("OpenCode plugin installed at {:?}", path);
    Ok(())
}

pub fn remove_opencode_plugin() -> Result<(), String> {
    if let Some(path) = plugin_path() {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove OpenCode plugin: {}", e))?;
        }
    }
    println!("OpenCode plugin removed");
    Ok(())
}

pub fn is_opencode_plugin_installed() -> bool {
    plugin_path().map(|p| p.exists()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_content_contains_port() {
        let content = plugin_content(19283);
        assert!(content.contains("const PORT = 19283;"));
        assert!(content.contains("source: \"opencode\""));
    }

    #[test]
    fn test_plugin_path_is_under_config() {
        if let Some(path) = plugin_path() {
            assert!(path.to_string_lossy().contains("opencode"));
            assert!(path.to_string_lossy().contains(PLUGIN_FILENAME));
        }
    }
}
