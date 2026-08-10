use crate::actions::search;
use crate::collectors::docker::collect;
use crate::collectors::{enrich, ports, processes, system};
use crate::config::Config;
use crate::models::{DevProcess, DockerContainer, PortBinding, SystemStats};
use anyhow::Result;

pub struct Snapshot {
    pub ports: Vec<PortBinding>,
    pub processes: Vec<DevProcess>,
    pub containers: Vec<DockerContainer>,
    pub docker_error: Option<String>,
    pub stats: SystemStats,
}

// TUI app state
pub struct App {
    pub config: Config,
    pub snapshot: Option<Snapshot>, // None before first refresh
    pub tab: Tab,
    pub selected_row: usize,
    pub list_offset: usize, // first visible row without header
    pub table_state: ratatui::widgets::TableState, // ratatui default table state
    pub should_quit: bool,
    pub needs_refresh: bool,
    pub last_error: Option<String>,
    pub input_mode: InputMode,
    pub search_query: String,
    pub search_match_index: usize,
    pub status_message: Option<String>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Ports,
    Processes,
    Docker,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InputMode {
    Normal,
    Search,
    ConfirmDockerRemove { id: String, name: String },
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            snapshot: None,
            tab: Tab::Ports,
            selected_row: 0,
            list_offset: 0,
            table_state: ratatui::widgets::TableState::default(),
            should_quit: false,
            needs_refresh: true,
            last_error: None,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            search_match_index: 0,
            status_message: None,
        }
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.selected_row = 0;
        self.list_offset = 0;
        self.search_query.clear();
        self.search_match_index = 0;
        self.input_mode = InputMode::Normal;
        self.clear_status();
    }

    pub fn reload_snapshot(&mut self) -> Result<()> {
        let mut ports = ports::collect(None)?;
        ports.retain(|binding| !self.config.ignored_ports.contains(&binding.port));
        let processes = processes::collect(true, &self.config.extra_dev_markers)?;
        let stats = system::collect()?;

        let (containers, docker_error) = match collect(self.config.docker_host()) {
            Ok(containers) => {
                enrich::attach_docker(&mut ports, &containers);
                (containers, None)
            }
            Err(error) => (Vec::new(), Some(error.to_string())),
        };

        self.snapshot = Some(Snapshot {
            ports,
            processes,
            containers,
            docker_error,
            stats,
        });

        self.last_error = None;

        self.clamp_selection_after_refresh();

        Ok(())
    }

    pub fn active_list_len(&self) -> usize {
        let Some(snapshot) = &self.snapshot else {
            return 0;
        };

        match self.tab {
            Tab::Ports => snapshot.ports.len(),
            Tab::Processes => snapshot.processes.len(),
            Tab::Docker => snapshot.containers.len(),
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.active_list_len();
        if len == 0 {
            return;
        }

        let max = len - 1;
        let next = (self.selected_row as isize + delta).clamp(0, max as isize) as usize;
        self.selected_row = next;
        self.table_state.select(Some(next));
    }

    pub fn clamp_selection_after_refresh(&mut self) {
        let len = self.active_list_len();
        if len == 0 {
            self.selected_row = 0;
            self.list_offset = 0;
            self.table_state.select(None);
            return;
        }
        if self.selected_row >= len {
            self.selected_row = len - 1;
        }
        self.table_state.select(Some(self.selected_row))
    }

    pub fn ensure_visible(&mut self, viewport_rows: usize) {
        if viewport_rows == 0 {
            return;
        }
        if self.selected_row < self.list_offset {
            self.list_offset = self.selected_row;
        } else if self.selected_row >= self.list_offset + viewport_rows {
            self.list_offset = self.selected_row - viewport_rows + 1;
        }
    }

    pub fn start_search(&mut self) {
        self.input_mode = InputMode::Search;
        self.search_query.clear();
        self.search_match_index = 0;
    }

    pub fn cancel_search(&mut self) {
        self.input_mode = InputMode::Normal;
        self.search_query.clear();
        self.search_match_index = 0;
    }

    pub fn push_search_char(&mut self, ch: char) {
        self.search_query.push(ch);
    }

    pub fn pop_search_char(&mut self) {
        self.search_query.pop();
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
            (self.search_match_index as isize + step).rem_euclid(count as isize) as usize
        };

        self.search_match_index = index;
        let row = matches[index];
        self.selected_row = row;
        self.table_state.select(Some(row));
    }

    pub fn select_search_status(&self) -> Option<String> {
        if self.search_query.trim().is_empty() {
            return None;
        }
        let matches = search::find_matches(self);
        if matches.is_empty() {
            return Some(format!("/{}", self.search_query) + "  (no matches)");
        }
        Some(format!(
            "/{}  [{}/{}]",
            self.search_query,
            self.search_match_index + 1,
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

    pub fn selected_process(&self) -> Option<&DevProcess> {
        if self.tab != Tab::Processes {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot.processes.get(self.selected_row)
    }

    pub fn kill_selected_process(&mut self) {
        let Some(process) = self.selected_process().cloned() else {
            self.set_status("no process selected");
            return;
        };

        match crate::actions::process::kill_process(process.pid) {
            Ok(()) => {
                self.set_status(format!(
                    "process killed {} (pid: {})",
                    process.name, process.pid,
                ));
                self.needs_refresh = true;
            }
            Err(err) => self.set_status(format!("killed process error {err}")),
        }
    }

    pub fn selected_container(&self) -> Option<&DockerContainer> {
        if self.tab != Tab::Docker {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot.containers.get(self.selected_row)
    }

    pub fn stop_selected_container(&mut self) {
        // if confirm remove is active - refresh
        self.cancel_pending_action();
        let Some(container) = self.selected_container().cloned() else {
            self.set_status("No container to select");
            return;
        };

        match crate::actions::docker::stop_container(&container.id, self.config.docker_host()) {
            Ok(()) => {
                self.set_status(format!("stopped {}", container.name));
                self.needs_refresh = true;
            }
            Err(error) => self.set_status(format!("stop failed container: {error}")),
        }
    }

    pub fn restart_selected_container(&mut self) {
        let Some(container) = self.selected_container().cloned() else {
            self.set_status("No container to select");
            return;
        };

        match crate::actions::docker::restart_container(&container.id, self.config.docker_host()) {
            Ok(()) => {
                self.set_status(format!("restarted {}", container.name));
                self.needs_refresh = true;
            }
            Err(error) => self.set_status(format!(
                "restart
             failed container: {error}"
            )),
        }
    }

    pub fn request_remove_selected_container(&mut self) {
        let Some(container) = self.selected_container().cloned() else {
            self.set_status("no container selected");
            return;
        };
        self.input_mode = InputMode::ConfirmDockerRemove {
            id: container.id,
            name: container.name,
        }
    }

    pub fn confirm_docker_remove(&mut self) {
        let InputMode::ConfirmDockerRemove { id, name } = self.input_mode.clone() else {
            return;
        };

        self.input_mode = InputMode::Normal;

        match crate::actions::docker::remove_container(&id, self.config.docker_host()) {
            Ok(()) => {
                self.set_status(format!("removed {name}"));
                self.needs_refresh = true;
            }
            Err(error) => self.set_status(format!("remove failed: {error}")),
        }
    }

    pub fn cancel_pending_action(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn selected_port(&self) -> Option<&PortBinding> {
        if self.tab != Tab::Ports {
            return None;
        }
        let snapshot = self.snapshot.as_ref()?;
        snapshot.ports.get(self.selected_row)
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
            if self.jump_to_process_by_pid(pid) {
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
        self.select_row(index);
        true
    }

    fn jump_to_process_by_pid(&mut self, pid: u32) -> bool {
        let Some(index) = self
            .snapshot
            .as_ref()
            .and_then(|s| s.processes.iter().position(|p| p.pid == pid))
        else {
            return false;
        };
        self.set_tab(Tab::Processes);
        self.select_row(index);
        true
    }

    fn select_row(&mut self, index: usize) {
        self.selected_row = index;
        self.list_offset = 0;
        self.table_state.select(Some(index));
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::models::Protocol;

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
            stats: empty_stats(),
        }
    }

    fn app_with_ports(n: usize) -> App {
        let mut app = App::new(Config::default());
        app.tab = Tab::Ports;
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
        app.tab = Tab::Ports;
        let mut binding = port(6379, "docker-proxy");
        binding.pid = Some(42);
        binding.container_name = Some("redis-dev".to_string());
        app.snapshot = Some(Snapshot {
            ports: vec![binding],
            processes: vec![process(42, "docker-proxy")],
            containers: vec![container("redis-dev", vec![6379])],
            docker_error: None,
            stats: empty_stats(),
        });

        app.jump_from_selected_port();

        assert_eq!(app.tab, Tab::Docker);
        assert_eq!(app.selected_row, 0);
        assert!(app.status_message.as_deref().unwrap().contains("redis-dev"))
    }

    #[test]
    fn jump_from_port_to_process_when_no_container() {
        let mut app = App::new(Config::default());
        app.tab = Tab::Ports;
        let mut binding = port(3000, "node");
        binding.pid = Some(100);
        app.snapshot = Some(Snapshot {
            ports: vec![binding],
            processes: vec![process(99, "other"), process(100, "node")],
            containers: vec![],
            docker_error: None,
            stats: empty_stats(),
        });

        app.jump_from_selected_port();
        assert_eq!(app.tab, Tab::Processes);
        assert_eq!(app.selected_row, 1)
    }

    #[test]
    fn jump_reports_when_process_not_in_dev_list() {
        let mut app = App::new(Config::default());
        app.tab = Tab::Ports;
        let mut binding = port(5353, "mDNSResponder");
        binding.pid = Some(1503);
        app.snapshot = Some(Snapshot {
            ports: vec![binding],
            processes: vec![], // DEV list empty / filtered
            containers: vec![],
            docker_error: None,
            stats: empty_stats(),
        });
        app.jump_from_selected_port();
        assert_eq!(app.tab, Tab::Ports);
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
        assert_eq!(app.selected_row, 0);
        app.move_selection(100);
        assert_eq!(app.selected_row, 2);
        app.move_selection(-1);
        assert_eq!(app.selected_row, 1);
    }

    #[test]
    fn move_selection_noop_when_empty() {
        let mut app = App::new(Config::default());
        app.move_selection(1);
        assert_eq!(app.selected_row, 0);
    }

    #[test]
    fn clamp_selection_after_refresh_when_row_out_of_range() {
        let mut app = app_with_ports(2);
        app.selected_row = 99;
        app.clamp_selection_after_refresh();
        assert_eq!(app.selected_row, 1);
    }

    #[test]
    fn clamp_selection_clears_when_list_empty() {
        let mut app = App::new(Config::default());
        app.selected_row = 105;
        app.list_offset = 2;

        app.clamp_selection_after_refresh();
        assert_eq!(app.selected_row, 0);
        assert_eq!(app.list_offset, 0);
    }

    #[test]
    fn ensure_visible_scrolls_down_and_up() {
        let mut app = app_with_ports(10);
        app.list_offset = 0;
        app.selected_row = 7;

        app.ensure_visible(5);
        assert_eq!(app.list_offset, 3);
        app.selected_row = 1;
        app.ensure_visible(5);
        assert_eq!(app.list_offset, 1);
    }

    #[test]
    fn set_tab_resets_selection_and_search() {
        let mut app = app_with_ports(3);
        app.selected_row = 2;
        app.list_offset = 1;
        app.search_match_index = 1;
        app.input_mode = InputMode::Search;

        app.set_tab(Tab::Docker);

        assert_eq!(app.tab, Tab::Docker);
        assert_eq!(app.selected_row, 0);
        assert_eq!(app.search_match_index, 0);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn apply_search_jumps_to_first_then_cycles() {
        let mut app = app_with_ports(0);
        app.snapshot = Some(Snapshot {
            ports: vec![port(8080, "node"), port(3000, "vite"), port(8081, "node")],
            processes: vec![],
            containers: vec![],
            docker_error: None,
            stats: empty_stats(),
        });
        app.search_query = "node".to_string();

        app.apply_search(0);
        assert_eq!(app.selected_row, 0);
        assert_eq!(app.search_match_index, 0);

        app.apply_search(1); // n
        assert_eq!(app.selected_row, 2);
        assert_eq!(app.search_match_index, 1);

        app.apply_search(1); // wrap
        assert_eq!(app.selected_row, 0);
        assert_eq!(app.search_match_index, 0);

        app.apply_search(-1); // N wrap backwards
        assert_eq!(app.selected_row, 2);
    }

    #[test]
    fn apply_search_noop_when_no_matches() {
        let mut app = app_with_ports(2);
        app.selected_row = 1;
        app.search_query = "zzz".to_string();

        app.apply_search(0);

        assert_eq!(app.selected_row, 1)
    }

    #[test]
    fn start_and_cancel_search() {
        let mut app = App::new(Config::default());
        app.search_query = "old".to_string();

        app.start_search();
        assert_eq!(app.input_mode, InputMode::Search);
        assert!(app.search_query.is_empty());

        app.push_search_char('a');
        app.push_search_char('b');

        assert_eq!(app.search_query, "ab");
        app.pop_search_char();
        assert_eq!(app.search_query, "a");

        app.cancel_search();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn select_search_status_formats() {
        let mut app = app_with_ports(0);
        app.snapshot = Some(Snapshot {
            ports: vec![port(8080, "node"), port(8081, "node")],
            processes: vec![],
            containers: vec![],
            docker_error: None,
            stats: empty_stats(),
        });
        assert!(app.select_search_status().is_none());
        app.search_query = "zzz".to_string();
        assert_eq!(
            app.select_search_status().as_deref(),
            Some("/zzz  (no matches)")
        );
        app.search_query = "node".to_string();
        app.search_match_index = 0;
        assert_eq!(app.select_search_status().as_deref(), Some("/node  [1/2]"));
    }
}
