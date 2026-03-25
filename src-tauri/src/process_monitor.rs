use sysinfo::{Pid, ProcessRefreshKind, System, UpdateKind};

pub struct ProcessMonitor {
    system: System,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        ProcessMonitor {
            system: System::new(),
        }
    }

    /// Find the PID of a `claude` process whose cwd matches the given path.
    /// Called once when a session is created.
    pub fn find_claude_pid(&mut self, project_path: &str) -> Option<u32> {
        if project_path.is_empty() {
            return None;
        }

        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cwd(UpdateKind::Always),
        );

        let target = normalize_path(project_path);

        for (pid, process) in self.system.processes() {
            let name = process.name().to_string_lossy().to_lowercase();
            if name != "claude" && name != "claude.exe" {
                continue;
            }

            let proc_cwd = match process.cwd() {
                Some(cwd) if !cwd.as_os_str().is_empty() => cwd,
                _ => continue,
            };

            if normalize_path(&proc_cwd.to_string_lossy()) == target {
                return Some(pid.as_u32());
            }
        }

        None
    }

    /// Check which PIDs from the input are dead. Returns the set of dead PIDs.
    pub fn find_dead_pids(&mut self, pids: &[u32]) -> Vec<u32> {
        if pids.is_empty() {
            return Vec::new();
        }
        let sysinfo_pids: Vec<Pid> = pids.iter().map(|p| Pid::from_u32(*p)).collect();
        self.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&sysinfo_pids),
            true,
            ProcessRefreshKind::nothing(),
        );
        pids.iter()
            .filter(|p| self.system.process(Pid::from_u32(**p)).is_none())
            .copied()
            .collect()
    }
}

/// Normalize a path for cross-format comparison.
/// Handles: Windows (`D:\foo`), Unix (`/home/foo`), and MSYS/Git Bash (`/d/foo`).
fn normalize_path(p: &str) -> String {
    let trimmed = p.trim_end_matches(['/', '\\']);

    let converted = if cfg!(windows) {
        convert_msys_path(trimmed)
    } else {
        trimmed.to_string()
    };

    converted.replace('\\', "/").to_lowercase()
}

/// Convert MSYS/Git Bash paths like `/d/foo` to `d:/foo`.
fn convert_msys_path(p: &str) -> String {
    let bytes = p.as_bytes();
    if bytes.len() >= 3
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && bytes[2] == b'/'
    {
        let drive = (bytes[1] as char).to_lowercase().next().unwrap();
        return format!("{}:/{}", drive, &p[3..]);
    }
    if bytes.len() == 2 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() {
        let drive = (bytes[1] as char).to_lowercase().next().unwrap();
        return format!("{}:/", drive);
    }
    p.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_windows() {
        assert_eq!(normalize_path("C:\\Users\\user\\project"), "c:/users/user/project");
        assert_eq!(normalize_path("D:\\Code\\MyApp\\"), "d:/code/myapp");
    }

    #[test]
    fn test_normalize_path_unix() {
        assert_eq!(normalize_path("/home/user/project/"), "/home/user/project");
        assert_eq!(normalize_path("/home/user/project"), "/home/user/project");
    }

    #[test]
    fn test_normalize_path_msys() {
        if cfg!(windows) {
            assert_eq!(normalize_path("/d/claude-pulse"), "d:/claude-pulse");
            assert_eq!(normalize_path("/c/Users/user/project"), "c:/users/user/project");
            assert_eq!(normalize_path("/D/Code"), "d:/code");
        }
    }

    #[test]
    fn test_convert_msys_path() {
        assert_eq!(convert_msys_path("/d/claude-pulse"), "d:/claude-pulse");
        assert_eq!(convert_msys_path("/C/Users/foo"), "c:/Users/foo");
        assert_eq!(convert_msys_path("/d"), "d:/");
        assert_eq!(convert_msys_path("/home/user"), "/home/user");
        assert_eq!(convert_msys_path("/usr/local"), "/usr/local");
    }

    #[test]
    fn test_windows_and_msys_match() {
        if cfg!(windows) {
            assert_eq!(
                normalize_path("/d/claude-pulse"),
                normalize_path("D:\\claude-pulse\\")
            );
        }
    }

    #[test]
    fn test_find_pid_empty_path() {
        let mut monitor = ProcessMonitor::new();
        assert_eq!(monitor.find_claude_pid(""), None);
    }

    #[test]
    fn test_find_dead_pids_nonexistent() {
        let mut monitor = ProcessMonitor::new();
        let dead = monitor.find_dead_pids(&[999999999]);
        assert_eq!(dead, vec![999999999]);
    }

    #[test]
    fn test_find_dead_pids_empty() {
        let mut monitor = ProcessMonitor::new();
        assert!(monitor.find_dead_pids(&[]).is_empty());
    }
}
