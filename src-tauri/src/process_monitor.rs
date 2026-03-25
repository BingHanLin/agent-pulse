use sysinfo::{Pid, ProcessRefreshKind, System};

pub struct ProcessMonitor {
    system: System,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        ProcessMonitor {
            system: System::new(),
        }
    }

    /// Check which PIDs from the input are dead. Returns the dead PIDs.
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

#[cfg(test)]
mod tests {
    use super::*;

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
