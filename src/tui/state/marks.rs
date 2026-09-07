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
            Tab::Processes => self.pids.clear(),
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
        alive_pids: HashSet<u32>,
        alive_volumes: HashSet<&str>,
    ) {
        self.containers
            .retain(|id| alive_containers.contains(id.as_str()));
        self.pids.retain(|pid| alive_pids.contains(pid));
        self.volumes
            .retain(|name| alive_volumes.contains(name.as_str()));
    }
}
