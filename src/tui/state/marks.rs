use crate::tui::state::nav::Tab;
use std::collections::HashSet;

#[derive(Debug, Default, Clone)]
pub struct Marks {
    pub containers: HashSet<String>,
    pub pids: HashSet<u32>,
    pub volumes: HashSet<String>,
}

impl Marks {
    pub fn toggle_container(&mut self, id: String) {
        if !self.containers.remove(&id) {
            self.containers.insert(id);
        }
    }

    pub fn toggle_pid(&mut self, pid: u32) {
        if !self.pids.remove(&pid) {
            self.pids.insert(pid);
        }
    }

    pub fn toggle_volumes(&mut self, volumes_name: String) {
        if !self.volumes.remove(&volumes_name) {
            self.volumes.insert(volumes_name);
        }
    }

    pub fn clear(&mut self, tab: Tab) {
        match tab {
            Tab::Docker => self.containers.clear(),
            Tab::DevProcesses | Tab::RegularProcesses => self.pids.clear(),
            Tab::Ports => {}
            Tab::Volumes => self.volumes.clear(),
        }
    }

    pub fn clear_all(&mut self) {
        self.containers.clear();
        self.pids.clear();
        self.volumes.clear();
    }

    pub fn mark_all_containers(&mut self, ids: impl IntoIterator<Item = String>) {
        self.containers = ids.into_iter().collect();
    }

    pub fn mark_all_pids(&mut self, pids: impl IntoIterator<Item = u32>) {
        self.pids = pids.into_iter().collect();
    }

    pub fn mark_all_volumes(&mut self, volumes: impl IntoIterator<Item = String>) {
        self.volumes = volumes.into_iter().collect();
    }

    pub fn prune(
        &mut self,
        alive_containers: HashSet<&str>,
        alive_dev_pids: HashSet<u32>,
        alive_regular_pids: HashSet<u32>,
        alive_volumes: HashSet<&str>,
    ) {
        self.containers
            .retain(|id| alive_containers.contains(id.as_str()));
        self.pids
            .retain(|pid| alive_dev_pids.contains(pid) || alive_regular_pids.contains(pid));
        self.volumes
            .retain(|name| alive_volumes.contains(name.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn toggle_pid_adds_and_removes() {
        let mut marks = Marks::default();
        marks.toggle_pid(10);
        assert!(marks.pids.contains(&10));
        marks.toggle_pid(10);
        assert!(!marks.pids.contains(&10));
    }

    #[test]
    fn clear_pids_on_either_process_tab() {
        let mut marks = Marks::default();
        marks.pids.insert(1);
        marks.containers.insert("c".into());

        marks.clear(Tab::DevProcesses);
        assert!(marks.pids.is_empty());
        assert!(marks.containers.contains("c"));

        marks.pids.insert(2);
        marks.clear(Tab::RegularProcesses);
        assert!(marks.pids.is_empty());
        assert!(marks.containers.contains("c"));
    }

    #[test]
    fn clear_ports_is_noop() {
        let mut marks = Marks::default();
        marks.pids.insert(1);
        marks.clear(Tab::Ports);
        assert!(marks.pids.contains(&1));
    }

    #[test]
    fn prune_keeps_pid_alive_in_dev_or_regular() {
        let mut marks = Marks::default();
        marks.pids.extend([1, 2, 3]);

        marks.prune(
            HashSet::new(),
            HashSet::from([1]),
            HashSet::from([2]),
            HashSet::new(),
        );

        assert_eq!(marks.pids, HashSet::from([1, 2]));
    }

    #[test]
    fn prune_drops_pid_missing_from_both_lists() {
        let mut marks = Marks::default();
        marks.pids.insert(99);

        marks.prune(HashSet::new(), HashSet::from([1]), HashSet::from([2]), HashSet::new());

        assert!(marks.pids.is_empty());
    }

    #[test]
    fn mark_all_pids_replaces_set() {
        let mut marks = Marks::default();
        marks.pids.insert(1);
        marks.mark_all_pids([7, 8]);
        assert_eq!(marks.pids, HashSet::from([7, 8]));
    }
}
