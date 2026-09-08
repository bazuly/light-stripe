use crate::actions::search;
use crate::config::Config;
use crate::models::{
    DevProcess, DockerContainer, DockerVolume, PortBinding, RegularProcess, Snapshot,
};
use crate::tui::collector_worker::ToWorker;
use crate::tui::state::marks::Marks;
use crate::tui::state::nav::Navigation;
use crate::tui::state::search::Search;
use crate::tui::state::worker::WorkerClient;
use std::collections::HashSet;

pub use crate::tui::state::nav::Tab;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InputMode {
    Normal,
    Search,
    ConfirmDockerRemove { targets: Vec<(String, String)> }, // id, name
    ConfirmProcessRemove { targets: Vec<(u32, String)> },   // pid, name
    ConfirmVolumeRemove { targets: Vec<String> },
}

// TUI app state
pub struct App {
    pub config: Config,
    pub snapshot: Option<Snapshot>,
    pub nav: Navigation,
    pub should_quit: bool,
    pub needs_refresh: bool,
    pub last_error: Option<String>,
    pub input_mode: InputMode,
    pub search: Search,
    pub status_message: Option<String>,
    pub marks: Marks,
    pub to_worker: WorkerClient,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            snapshot: None,
            nav: Navigation::new(),
            should_quit: false,
            needs_refresh: true,
            last_error: None,
            input_mode: InputMode::Normal,
            search: Search::default(),
            status_message: None,
            marks: Marks::default(),
            to_worker: WorkerClient::default(),
        }
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.nav.set_tab(tab);
        self.search.clear();
        self.input_mode = InputMode::Normal;
        self.clear_status();
    }

    pub fn current_tab(&self) -> Tab {
        self.nav.tab
    }

    pub fn apply_snapshot(&mut self, snapshot: Snapshot, warning: Option<String>) {
        self.snapshot = Some(snapshot);
        self.last_error = None;
        if let Some(msg) = warning {
            self.set_status(msg);
        }
        self.prune_marks_after_refresh();
        self.clamp_selection_after_refresh();
    }

    fn enqueue(&mut self, cmd: ToWorker, pending_status: &str) {
        match self.to_worker.enqueue(cmd) {
            Ok(()) => self.set_status(pending_status),
            Err(msg) => self.set_status(msg),
        }
    }

    pub fn active_list_len(&self) -> usize {
        let Some(snapshot) = &self.snapshot else {
            return 0;
        };

        match self.nav.tab {
            Tab::Ports => snapshot.ports.len(),
            Tab::DevProcesses => snapshot.dev_processes.len(),
            Tab::RegularProcesses => snapshot.regular_processes.len(),
            Tab::Docker => snapshot.containers.len(),
            Tab::Volumes => snapshot.volumes.len(),
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.active_list_len();
        self.nav.move_by(delta, len)
    }

    pub fn toggle_mark_current(&mut self) {
        match self.nav.tab {
            Tab::Docker => {
                let Some(c) = self.selected_container() else {
                    return;
                };
                let id = c.id.clone();
                self.marks.toggle_container(id);
            }

            Tab::DevProcesses => {
                let Some(c) = self.selected_dev_process() else {
                    return;
                };
                let pid = c.pid;
                self.marks.toggle_pid(pid);
            }

            Tab::RegularProcesses => {
                let Some(c) = self.selected_regular_process() else {
                    return;
                };
                let pid = c.pid;
                self.marks.toggle_pid(pid);
            }

            Tab::Volumes => {
                let Some(v) = self.selected_volume() else {
                    return;
                };
                let name = v.name.clone();
                self.marks.toggle_volumes(name);
            }

            Tab::Ports => {}
        }
    }

    pub fn mark_all(&mut self) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        match self.nav.tab {
            Tab::Docker => {
                self.marks
                    .mark_all_containers(snapshot.containers.iter().map(|c| c.id.clone()));
            }
            Tab::DevProcesses => {
                self.marks
                    .mark_all_pids(snapshot.dev_processes.iter().map(|p| p.pid));
            }
            Tab::RegularProcesses => {
                self.marks
                    .mark_all_pids(snapshot.regular_processes.iter().map(|p| p.pid));
            }
            Tab::Volumes => {
                self.marks
                    .mark_all_volumes(snapshot.volumes.iter().map(|v| v.name.clone()));
            }
            Tab::Ports => {}
        }
    }

    pub fn unmark_all(&mut self) {
        self.marks.clear(self.nav.tab)
    }

    /// After refresh, remove items (containers, pids, volumes) which are not in snapshot
    pub fn prune_marks_after_refresh(&mut self) {
        let Some(snapshot) = &self.snapshot else {
            self.marks.clear_all();
            return;
        };

        let alive_containers: HashSet<&str> =
            snapshot.containers.iter().map(|c| c.id.as_str()).collect();
        let alive_dev_pids: HashSet<u32> = snapshot.dev_processes.iter().map(|p| p.pid).collect();
        let alive_regular_pids: HashSet<u32> =
            snapshot.regular_processes.iter().map(|p| p.pid).collect();
        let alive_volumes: HashSet<&str> =
            snapshot.volumes.iter().map(|v| v.name.as_str()).collect();

        self.marks.prune(
            alive_containers,
            alive_dev_pids,
            alive_regular_pids,
            alive_volumes,
        );
    }

    pub fn clamp_selection_after_refresh(&mut self) {
        let len = self.active_list_len();
        self.nav.clamp_to_len(len);
    }

    pub fn start_search(&mut self) {
        self.input_mode = InputMode::Search;
        self.search.clear();
        self.search.match_index = 0;
    }

    pub fn cancel_search(&mut self) {
        self.input_mode = InputMode::Normal;
        self.search.clear();
        self.search.match_index = 0;
    }

    pub fn apply_search(&mut self, step: isize) {
        let matches = search::find_matches(self);
        if matches.is_empty() {
            return;
        }

        let count = matches.len();
        let index = if step == 0 {
            0
        } else {
            (self.search.match_index as isize + step).rem_euclid(count as isize) as usize
        };

        self.search.match_index = index;
        self.nav.select_row(matches[index]);
    }

    pub fn select_search_status(&self) -> Option<String> {
        if self.search.query.trim().is_empty() {
            return None;
        }
        let matches = search::find_matches(self);
        if matches.is_empty() {
            return Some(format!("/{}", self.search.query) + "  (no matches)");
        }
        Some(format!(
            "/{}  [{}/{}]",
            self.search.query,
            self.search.match_index + 1,
            matches.len()
        ))
    }

    // impl Into<String> in Rust means: receive any method
    // that can be mute to string
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn selected_dev_process(&self) -> Option<&DevProcess> {
        if self.nav.tab != Tab::DevProcesses {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot.dev_processes.get(self.nav.selected_row)
    }

    pub fn selected_regular_process(&self) -> Option<&RegularProcess> {
        if self.nav.tab != Tab::RegularProcesses {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot.regular_processes.get(self.nav.selected_row)
    }

    pub fn selected_container(&self) -> Option<&DockerContainer> {
        if self.nav.tab != Tab::Docker {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot.containers.get(self.nav.selected_row)
    }

    pub fn selected_volume(&self) -> Option<&DockerVolume> {
        if self.nav.tab != Tab::Volumes {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot.volumes.get(self.nav.selected_row)
    }

    pub fn stop_selected_container(&mut self) {
        self.cancel_pending_action();
        let tab = Tab::Docker;
        let targets = self.docker_action_targets();
        if targets.is_empty() {
            self.set_status("no container selected");
            return;
        }
        let ids: Vec<String> = targets.into_iter().map(|(id, _)| id).collect();
        self.marks.clear(tab);
        self.enqueue(ToWorker::StopContainers { ids }, "stopping container");
    }

    pub fn restart_selected_container(&mut self) {
        self.cancel_pending_action();
        let tab = Tab::Docker;
        let targets = self.docker_action_targets();
        if targets.is_empty() {
            self.set_status("no container selected");
            return;
        }
        let ids: Vec<String> = targets.into_iter().map(|(id, _)| id).collect();
        self.marks.clear(tab);
        self.enqueue(ToWorker::RestartContainers { ids }, "restarting container");
    }

    pub fn confirm_docker_remove(&mut self) {
        let tab = Tab::Docker;
        let InputMode::ConfirmDockerRemove { targets } = self.input_mode.clone() else {
            return;
        };

        self.input_mode = InputMode::Normal;
        let ids: Vec<String> = targets.into_iter().map(|(id, _)| id).collect();
        self.marks.clear(tab);
        self.enqueue(
            ToWorker::RemoveContainers { ids },
            "removing docker-container…",
        );
    }

    pub fn request_kill_selected_process(&mut self) {
        self.cancel_pending_action();
        let targets = self.process_action_targets();
        if targets.is_empty() {
            self.set_status("no process selected");
            return;
        }
        self.input_mode = InputMode::ConfirmProcessRemove { targets };
    }

    pub fn confirm_kill_selected_process(&mut self, tab: Tab) {
        let InputMode::ConfirmProcessRemove { targets } = self.input_mode.clone() else {
            return;
        };
        self.input_mode = InputMode::Normal;
        let pids: Vec<u32> = targets.into_iter().map(|(pid, _)| pid).collect();
        self.marks.clear(tab);

        self.enqueue(ToWorker::KillProcesses { pids }, "killing…");
    }

    pub fn request_remove_selected_container(&mut self) {
        let targets = self.docker_action_targets();
        if targets.is_empty() {
            self.set_status("no containers selected");
            return;
        }
        self.input_mode = InputMode::ConfirmDockerRemove { targets }
    }

    pub fn cancel_pending_action(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn request_remove_selected_volumes(&mut self) {
        let targets = self.volume_action_targets();
        if targets.is_empty() {
            self.set_status("no volume selected");
            return;
        }

        if let Some(snapshot) = &self.snapshot {
            let in_use: Vec<&str> = targets
                .iter()
                .filter(|name| {
                    snapshot
                        .volumes
                        .iter()
                        .any(|volume| volume.name == **name && volume.in_use)
                })
                .map(String::as_str)
                .collect();
            if !in_use.is_empty() {
                self.set_status(format!(
                    "refusing to delete in-use volumes: {}",
                    in_use.join(", ")
                ));
                return;
            }
        }

        self.input_mode = InputMode::ConfirmVolumeRemove { targets };
    }

    pub fn confirm_volume_remove(&mut self) {
        let InputMode::ConfirmVolumeRemove { targets } = self.input_mode.clone() else {
            return;
        };
        self.input_mode = InputMode::Normal;
        self.marks.volumes.clear();
        self.enqueue(
            ToWorker::RemoveVolumes { names: targets },
            "removing volumes…",
        );
    }

    pub fn jump_from_selected_volume(&mut self) {
        let Some(volume) = self.selected_volume().cloned() else {
            self.set_status("no volume selected");
            return;
        };

        if volume.container_names.is_empty() {
            self.set_status(format!(
                "volume {} is not used by any container",
                volume.name
            ));
            return;
        }

        let target = volume.container_names[0].clone();
        if self.jump_to_container_by_name(&target) {
            let extra = if volume.container_names.len() > 1 {
                format!(" ({} linked)", volume.container_names.len())
            } else {
                String::new()
            };
            self.set_status(format!("jumped to container {target}{extra}"));
        } else {
            self.set_status(format!("container {target} not in Docker list"));
        }
    }

    pub fn selected_port(&self) -> Option<&PortBinding> {
        if self.nav.tab != Tab::Ports {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot.ports.get(self.nav.selected_row)
    }

    pub fn jump_from_selected_port(&mut self) {
        let Some(binding) = self.selected_port().cloned() else {
            self.set_status("no port selected");
            return;
        };
        // Prefer Docker when OWNER is a container (same as enrich preference).
        if let Some(name) = binding.container_name.as_deref() {
            if self.jump_to_container_by_name(name) {
                self.set_status(format!("jumped to container {name}"));
                return;
            }
            // If Docker tab missing this container — fall through to process.
        };

        if let Some(pid) = binding.pid {
            if self.jump_to_dev_process_by_pid(pid) {
                let label = binding.process_name.as_deref().unwrap_or("process");
                self.set_status(format!("jumped to {label} (pid: {pid})"));
                return;
            } else {
                let label = binding.process_name.as_deref().unwrap_or("process");
                self.set_status(format!(
                    "{label} (pid {pid}) is not in DEV Processes list, unable to reach"
                ));
            }
            return;
        }
    }

    fn jump_to_container_by_name(&mut self, name: &str) -> bool {
        let Some(index) = self
            .snapshot
            .as_ref()
            .and_then(|s| s.containers.iter().position(|c| c.name == name))
        else {
            return false;
        };
        self.set_tab(Tab::Docker);
        self.nav.select_row(index);
        true
    }

    fn jump_to_dev_process_by_pid(&mut self, pid: u32) -> bool {
        let Some(index) = self
            .snapshot
            .as_ref()
            .and_then(|s| s.dev_processes.iter().position(|p| p.pid == pid))
        else {
            return false;
        };
        self.set_tab(Tab::DevProcesses);
        self.nav.select_row(index);
        true
    }

    // retrieve docker-containers id's and names
    fn docker_action_targets(&self) -> Vec<(String, String)> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };

        if !self.marks.containers.is_empty() {
            return snapshot
                .containers
                .iter()
                .filter(|c| self.marks.containers.contains(&c.id))
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect();
        }

        self.selected_container()
            .map(|c| vec![(c.id.clone(), c.name.clone())])
            .unwrap_or_default()
    }

    fn process_action_targets(&self) -> Vec<(u32, String)> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };

        if !self.marks.pids.is_empty() {
            return snapshot
                .processes
                .iter()
                .filter(|process| self.marks.pids.contains(&process.pid))
                .map(|process| (process.pid.clone(), process.name.clone()))
                .collect();
        }

        self.selected_process()
            .map(|p| vec![(p.pid, p.name.clone())])
            .unwrap_or_default()
    }

    fn volume_action_targets(&self) -> Vec<String> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };

        if !self.marks.volumes.is_empty() {
            return snapshot
                .volumes
                .iter()
                .filter(|volume| self.marks.volumes.contains(&volume.name))
                .map(|volume| volume.name.clone())
                .collect();
        }

        self.selected_volume()
            .map(|volume| vec![volume.name.clone()])
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::models::{Protocol, SystemStats};

    fn empty_stats() -> SystemStats {
        SystemStats {
            total_memory: 0,
            used_memory: 0,
            global_cpu_usage: 0.0,
            cpu_temp_c: None,
            gpu_temp_c: None,
        }
    }

    fn port(port: u16, process_name: &str) -> PortBinding {
        PortBinding {
            port,
            protocol: Protocol::Tcp,
            address: "127.0.0.1".to_string(),
            pid: Some(1),
            process_name: Some(process_name.to_string()),
            container_name: None,
            container_image: None,
        }
    }

    fn snapshot_ports(n: usize) -> Snapshot {
        let ports = (0..n).map(|i| port(8000 + i as u16, "node")).collect();
        Snapshot {
            ports,
            processes: vec![],
            containers: vec![],
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        }
    }

    fn app_with_ports(n: usize) -> App {
        let mut app = App::new(Config::default());
        app.nav.tab = Tab::Ports;
        app.snapshot = Some(snapshot_ports(n));
        app.needs_refresh = false;
        app
    }

    fn container(name: &str, host_ports: Vec<u16>) -> DockerContainer {
        DockerContainer {
            id: format!("id-{name}"),
            name: name.to_string(),
            image: format!("{name}:latest"),
            status: "running".to_string(),
            host_ports,
            cpu_percent: None,
            memory_bytes: None,
        }
    }

    fn process(pid: u32, name: &str) -> DevProcess {
        DevProcess {
            pid,
            name: name.to_string(),
            cmdline: name.to_string(),
            memory_bytes: 0,
            cpu_usage: 0.0,
            is_dev: true,
        }
    }

    #[test]
    fn jump_from_port_to_container() {
        let mut app = App::new(Config::default());
        app.nav.tab = Tab::Ports;
        let mut binding = port(6379, "docker-proxy");
        binding.pid = Some(42);
        binding.container_name = Some("redis-dev".to_string());
        app.snapshot = Some(Snapshot {
            ports: vec![binding],
            processes: vec![process(42, "docker-proxy")],
            containers: vec![container("redis-dev", vec![6379])],
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        });

        app.jump_from_selected_port();

        assert_eq!(app.nav.tab, Tab::Docker);
        assert_eq!(app.nav.selected_row, 0);
        assert!(app.status_message.as_deref().unwrap().contains("redis-dev"))
    }

    #[test]
    fn jump_from_port_to_process_when_no_container() {
        let mut app = App::new(Config::default());
        app.nav.tab = Tab::Ports;
        let mut binding = port(3000, "node");
        binding.pid = Some(100);
        app.snapshot = Some(Snapshot {
            ports: vec![binding],
            processes: vec![process(99, "other"), process(100, "node")],
            containers: vec![],
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        });

        app.jump_from_selected_port();
        assert_eq!(app.nav.tab, Tab::Processes);
        assert_eq!(app.nav.selected_row, 1)
    }

    #[test]
    fn jump_reports_when_process_not_in_dev_list() {
        let mut app = App::new(Config::default());
        app.nav.tab = Tab::Ports;
        let mut binding = port(5353, "mDNSResponder");
        binding.pid = Some(1503);
        app.snapshot = Some(Snapshot {
            ports: vec![binding],
            processes: vec![], // DEV list empty / filtered
            containers: vec![],
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        });
        app.jump_from_selected_port();
        assert_eq!(app.nav.tab, Tab::Ports);
        assert!(
            app.status_message
                .as_deref()
                .unwrap()
                .contains("not in DEV Processes")
        );
    }

    #[test]
    fn move_selection_clamps_at_bounds() {
        let mut app = app_with_ports(3);
        app.move_selection(-1);
        assert_eq!(app.nav.selected_row, 0);
        app.move_selection(100);
        assert_eq!(app.nav.selected_row, 2);
        app.move_selection(-1);
        assert_eq!(app.nav.selected_row, 1);
    }

    #[test]
    fn move_selection_noop_when_empty() {
        let mut app = App::new(Config::default());
        app.move_selection(1);
        assert_eq!(app.nav.selected_row, 0);
    }

    #[test]
    fn clamp_selection_after_refresh_when_row_out_of_range() {
        let mut app = app_with_ports(2);
        app.nav.selected_row = 99;
        app.clamp_selection_after_refresh();
        assert_eq!(app.nav.selected_row, 1);
    }

    #[test]
    fn clamp_selection_clears_when_list_empty() {
        let mut app = App::new(Config::default());
        app.nav.selected_row = 105;
        app.nav.list_offset = 2;

        app.clamp_selection_after_refresh();
        assert_eq!(app.nav.selected_row, 0);
        assert_eq!(app.nav.list_offset, 0);
    }

    #[test]
    fn ensure_visible_scrolls_down_and_up() {
        let mut app = app_with_ports(10);
        app.nav.list_offset = 0;
        app.nav.selected_row = 7;

        app.nav.ensure_visible(5);
        assert_eq!(app.nav.list_offset, 3);
        app.nav.selected_row = 1;
        app.nav.ensure_visible(5);
        assert_eq!(app.nav.list_offset, 1);
    }

    #[test]
    fn set_tab_resets_selection_and_search() {
        let mut app = app_with_ports(3);
        app.nav.selected_row = 2;
        app.nav.list_offset = 1;
        app.search.match_index = 1;
        app.input_mode = InputMode::Search;

        app.set_tab(Tab::Docker);

        assert_eq!(app.nav.tab, Tab::Docker);
        assert_eq!(app.nav.selected_row, 0);
        assert_eq!(app.search.match_index, 0);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.search.query.is_empty());
    }

    #[test]
    fn apply_search_jumps_to_first_then_cycles() {
        let mut app = app_with_ports(0);
        app.snapshot = Some(Snapshot {
            ports: vec![port(8080, "node"), port(3000, "vite"), port(8081, "node")],
            processes: vec![],
            containers: vec![],
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        });
        app.search.query = "node".to_string();

        app.apply_search(0);
        assert_eq!(app.nav.selected_row, 0);
        assert_eq!(app.search.match_index, 0);

        app.apply_search(1); // n
        assert_eq!(app.nav.selected_row, 2);
        assert_eq!(app.search.match_index, 1);

        app.apply_search(1); // wrap
        assert_eq!(app.nav.selected_row, 0);
        assert_eq!(app.search.match_index, 0);

        app.apply_search(-1); // N wrap backwards
        assert_eq!(app.nav.selected_row, 2);
    }

    #[test]
    fn apply_search_noop_when_no_matches() {
        let mut app = app_with_ports(2);
        app.nav.selected_row = 1;
        app.search.query = "zzz".to_string();

        app.apply_search(0);

        assert_eq!(app.nav.selected_row, 1)
    }

    #[test]
    fn start_and_cancel_search() {
        let mut app = App::new(Config::default());
        app.search.query = "old".to_string();

        app.start_search();
        assert_eq!(app.input_mode, InputMode::Search);
        assert!(app.search.query.is_empty());

        app.search.push_char('a');
        app.search.push_char('b');

        assert_eq!(app.search.query, "ab");
        app.search.pop_char();
        assert_eq!(app.search.query, "a");

        app.cancel_search();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.search.query.is_empty());
    }

    #[test]
    fn select_search_status_formats() {
        let mut app = app_with_ports(0);
        app.snapshot = Some(Snapshot {
            ports: vec![port(8080, "node"), port(8081, "node")],
            processes: vec![],
            containers: vec![],
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        });
        assert!(app.select_search_status().is_none());
        app.search.query = "zzz".to_string();
        assert_eq!(
            app.select_search_status().as_deref(),
            Some("/zzz  (no matches)")
        );
        app.search.query = "node".to_string();
        app.search.match_index = 0;
        assert_eq!(app.select_search_status().as_deref(), Some("/node  [1/2]"));
    }

    #[test]
    fn prune_drops_missing_ids() {
        let mut app = App::new(Config::default());
        app.marks.containers.insert("gone".into());
        app.snapshot = Some(Snapshot {
            ports: vec![],
            processes: vec![],
            containers: vec![],
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        });
        app.prune_marks_after_refresh();
        assert!(app.marks.containers.is_empty())
    }
}
