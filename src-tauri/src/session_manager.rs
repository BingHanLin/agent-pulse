use crate::process_monitor::ProcessMonitor;
use crate::webhook_server::HookEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const REMOVE_TIMEOUT_SECS: u64 = 5 * 60; // 5 minutes — fallback for sessions without PID

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionState {
    Idle,
    Working,
    WaitingForUser,
}

impl SessionState {
    pub fn priority(&self) -> u8 {
        match self {
            SessionState::WaitingForUser => 3,
            SessionState::Working => 2,
            SessionState::Idle => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub project_path: String,
    pub project_name: String,
    pub state: SessionState,
    pub start_time_ms: u64,
    pub last_activity_ms: u64,
    pub last_prompt: Option<String>,
    pub last_tool_name: Option<String>,
    pub is_active: bool,
    pub pid: Option<u32>,
    pub source: String,
}

#[derive(Debug, Clone)]
struct Session {
    id: String,
    project_path: String,
    project_name: String,
    state: SessionState,
    start_time_ms: u64,
    last_activity: Instant,
    last_activity_ms: u64,
    last_prompt: Option<String>,
    last_tool_name: Option<String>,
    pid: Option<u32>,
    source: String,
}

impl Session {
    fn new(id: String, cwd: Option<String>, pid: Option<u32>, source: String) -> Self {
        let project_path = cwd.unwrap_or_default();
        let project_name = extract_project_name(&project_path);
        let now_ms = current_time_ms();
        Session {
            id,
            project_path,
            project_name,
            state: SessionState::Idle,
            start_time_ms: now_ms,
            last_activity: Instant::now(),
            last_activity_ms: now_ms,
            last_prompt: None,
            last_tool_name: None,
            pid,
            source,
        }
    }

    fn touch(&mut self) {
        self.last_activity = Instant::now();
        self.last_activity_ms = current_time_ms();
    }

    fn to_info(&self, active_id: &Option<String>) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            project_path: self.project_path.clone(),
            project_name: self.project_name.clone(),
            state: self.state.clone(),
            start_time_ms: self.start_time_ms,
            last_activity_ms: self.last_activity_ms,
            last_prompt: self.last_prompt.clone(),
            last_tool_name: self.last_tool_name.clone(),
            is_active: active_id.as_ref() == Some(&self.id),
            pid: self.pid,
            source: self.source.clone(),
        }
    }
}

fn extract_project_name(path: &str) -> String {
    if path.is_empty() {
        return "Unknown".to_string();
    }
    let path = path.trim_end_matches(['/', '\\']);
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or("Unknown")
        .to_string()
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    selected_session: Arc<Mutex<Option<String>>>,
    process_monitor: Arc<Mutex<ProcessMonitor>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            selected_session: Arc::new(Mutex::new(None)),
            process_monitor: Arc::new(Mutex::new(ProcessMonitor::new())),
        }
    }

    pub fn handle_event(&self, event: &HookEvent) -> bool {
        let name = event.hook_event_name.as_str();
        // Treat 0 and 1 as invalid (MSYS init PID on Windows)
        let valid_pid = event.pid.filter(|&p| p > 1);
        let mut sessions = self.sessions.lock().unwrap();

        // Auto-create session if it doesn't exist (for any event type)
        if !sessions.contains_key(&event.session_id) && name != "SessionEnd" {
            let source = event.source.clone().unwrap_or_else(|| "unknown".to_string());
            let session = Session::new(
                event.session_id.clone(),
                event.cwd.clone(),
                valid_pid,
                source,
            );
            sessions.insert(event.session_id.clone(), session);
        }

        // Backfill PID from event if session doesn't have one yet
        if let Some(event_pid) = valid_pid {
            if let Some(session) = sessions.get_mut(&event.session_id) {
                if session.pid.is_none() {
                    session.pid = Some(event_pid);
                }
            }
        }

        let changed = match name {
            "SessionStart" => {
                let source = event.source.clone().unwrap_or_else(|| "unknown".to_string());
                let session = Session::new(
                    event.session_id.clone(),
                    event.cwd.clone(),
                    valid_pid,
                    source,
                );
                sessions.insert(event.session_id.clone(), session);
                true
            }
            "SessionEnd" => {
                sessions.remove(&event.session_id);
                let mut selected = self.selected_session.lock().unwrap();
                if selected.as_ref() == Some(&event.session_id) {
                    *selected = None;
                }
                true
            }
            "UserPromptSubmit" => {
                if let Some(session) = sessions.get_mut(&event.session_id) {
                    session.state = SessionState::Working;
                    session.touch();
                    if let Some(prompt) = &event.prompt {
                        session.last_prompt = Some(prompt.clone());
                    }
                    true
                } else {
                    false
                }
            }
            "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => {
                if let Some(session) = sessions.get_mut(&event.session_id) {
                    session.state = SessionState::Working;
                    session.touch();
                    if let Some(tool) = &event.tool_name {
                        session.last_tool_name = Some(tool.clone());
                    }
                    true
                } else {
                    false
                }
            }
            "PermissionRequest" => {
                if let Some(session) = sessions.get_mut(&event.session_id) {
                    session.state = SessionState::WaitingForUser;
                    session.touch();
                    true
                } else {
                    false
                }
            }
            "Stop" => {
                if let Some(session) = sessions.get_mut(&event.session_id) {
                    session.state = SessionState::Idle;
                    session.touch();
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        changed
    }

    /// Check for stale/dead sessions. Returns true if any sessions changed.
    pub fn check_staleness(&self) -> bool {
        // Collect PIDs to check before locking sessions
        let pids_to_check: Vec<(String, u32)> = {
            let sessions = self.sessions.lock().unwrap();
            sessions
                .iter()
                .filter_map(|(id, s)| s.pid.map(|pid| (id.clone(), pid)))
                .collect()
        };

        // Batch-check which PIDs are dead (single syscall)
        let dead_pid_set: std::collections::HashSet<u32> =
            if let Ok(mut monitor) = self.process_monitor.lock() {
                let all_pids: Vec<u32> = pids_to_check.iter().map(|(_, pid)| *pid).collect();
                monitor.find_dead_pids(&all_pids).into_iter().collect()
            } else {
                std::collections::HashSet::new()
            };

        let mut sessions = self.sessions.lock().unwrap();
        let mut changed = false;
        let mut to_remove = Vec::new();

        for (id, session) in sessions.iter_mut() {
            let elapsed = session.last_activity.elapsed();

            // PID-based removal: process is confirmed dead
            if session.pid.is_some_and(|pid| dead_pid_set.contains(&pid)) {
                to_remove.push(id.clone());
                changed = true;
                continue;
            }

            // Fallback timeout-based removal (for sessions without a resolved PID)
            if session.pid.is_none() && elapsed > Duration::from_secs(REMOVE_TIMEOUT_SECS) {
                to_remove.push(id.clone());
                changed = true;
            }
        }

        for id in to_remove {
            sessions.remove(&id);
            let mut selected = self.selected_session.lock().unwrap();
            if selected.as_ref() == Some(&id) {
                *selected = None;
            }
        }

        changed
    }

    pub fn get_sessions(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.lock().unwrap();
        let selected = self.selected_session.lock().unwrap();
        let active_id = self.resolve_active_id(&sessions, &selected);

        let mut infos: Vec<SessionInfo> =
            sessions.values().map(|s| s.to_info(&active_id)).collect();
        infos.sort_by(|a, b| b.last_activity_ms.cmp(&a.last_activity_ms));
        infos
    }

    fn resolve_active_id(
        &self,
        sessions: &HashMap<String, Session>,
        selected: &Option<String>,
    ) -> Option<String> {
        // Priority: user-selected > most active running > most recent
        if let Some(id) = selected {
            if sessions.contains_key(id) {
                return Some(id.clone());
            }
        }

        // Find highest priority running session
        sessions
            .values()
            .filter(|s| s.state == SessionState::Working || s.state == SessionState::WaitingForUser)
            .max_by_key(|s| (s.state.priority(), s.last_activity_ms))
            .or_else(|| sessions.values().max_by_key(|s| s.last_activity_ms))
            .map(|s| s.id.clone())
    }

    pub fn session_count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }

    /// Returns true if a Stop event just occurred (for playing sound)
    pub fn is_stop_event(event: &HookEvent) -> bool {
        event.hook_event_name == "Stop"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(session_id: &str, name: &str, cwd: Option<&str>) -> HookEvent {
        HookEvent {
            session_id: session_id.to_string(),
            hook_event_name: name.to_string(),
            cwd: cwd.map(|s| s.to_string()),
            tool_name: None,
            notification_type: None,
            prompt: None,
            pid: None,
            source: None,
        }
    }

    #[test]
    fn test_session_start() {
        let mgr = SessionManager::new();
        let event = make_event("s1", "SessionStart", Some("/home/user/project"));
        mgr.handle_event(&event);

        let sessions = mgr.get_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].state, SessionState::Idle);
        assert_eq!(sessions[0].project_name, "project");
    }

    #[test]
    fn test_state_transitions() {
        let mgr = SessionManager::new();
        mgr.handle_event(&make_event("s1", "SessionStart", Some("/project")));

        mgr.handle_event(&make_event("s1", "UserPromptSubmit", None));
        assert_eq!(mgr.get_sessions()[0].state, SessionState::Working);

        mgr.handle_event(&make_event("s1", "PermissionRequest", None));
        assert_eq!(mgr.get_sessions()[0].state, SessionState::WaitingForUser);

        mgr.handle_event(&make_event("s1", "Stop", None));
        assert_eq!(mgr.get_sessions()[0].state, SessionState::Idle);
    }

    #[test]
    fn test_session_end_removes() {
        let mgr = SessionManager::new();
        mgr.handle_event(&make_event("s1", "SessionStart", Some("/project")));
        assert_eq!(mgr.session_count(), 1);

        mgr.handle_event(&make_event("s1", "SessionEnd", None));
        assert_eq!(mgr.session_count(), 0);
    }

    #[test]
    fn test_multiple_sessions() {
        let mgr = SessionManager::new();
        mgr.handle_event(&make_event("s1", "SessionStart", Some("/project-a")));
        mgr.handle_event(&make_event("s2", "SessionStart", Some("/project-b")));
        assert_eq!(mgr.session_count(), 2);
    }

    #[test]
    fn test_extract_project_name() {
        assert_eq!(extract_project_name("/home/user/project"), "project");
        assert_eq!(extract_project_name("C:\\Users\\user\\project"), "project");
        assert_eq!(extract_project_name("/home/user/project/"), "project");
        assert_eq!(extract_project_name(""), "Unknown");
    }

    #[test]
    fn test_auto_create_on_prompt() {
        let mgr = SessionManager::new();
        // No SessionStart, but get a UserPromptSubmit
        let mut event = make_event("s1", "UserPromptSubmit", Some("/project"));
        event.prompt = Some("hello".to_string());
        mgr.handle_event(&event);

        assert_eq!(mgr.session_count(), 1);
        assert_eq!(mgr.get_sessions()[0].state, SessionState::Working);
    }
}
