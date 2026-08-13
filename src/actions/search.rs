use crate::models::{DevProcess, DockerContainer, DockerVolume, PortBinding};
use crate::output::table::format_port_owner;
use crate::tui::app::{App, Tab};

pub fn find_matches(app: &App) -> Vec<usize> {
    let Some(snapshot) = &app.snapshot else {
        return Vec::new();
    };

    let query = app.search_query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let query = query.to_lowercase();

    match app.tab {
        Tab::Ports => snapshot
            .ports
            .iter()
            .enumerate()
            .filter(|(_, binding)| port_matches(binding, &query))
            .map(|(index, _)| index)
            .collect(),
        Tab::Processes => snapshot
            .processes
            .iter()
            .enumerate()
            .filter(|(_, process)| process_matches(process, &query))
            .map(|(index, _)| index)
            .collect(),
        Tab::Docker => snapshot
            .containers
            .iter()
            .enumerate()
            .filter(|(_, container)| container_matches(container, &query))
            .map(|(index, _)| index)
            .collect(),
        Tab::Volumes => snapshot
            .volumes
            .iter()
            .enumerate()
            .filter(|(_, volume)| volume_matches(volume, &query))
            .map(|(index, _)| index)
            .collect(),
    }
}

fn port_matches(binding: &PortBinding, query: &str) -> bool {
    binding.port.to_string().contains(query)
        || binding.address.to_ascii_lowercase().contains(query)
        || format_port_owner(binding).contains(query)
        || binding
            .pid
            .map(|pid| pid.to_string().contains(query))
            .unwrap_or(false)
}

fn process_matches(process: &DevProcess, query: &str) -> bool {
    process.pid.to_string().contains(query)
        || process.name.to_ascii_lowercase().contains(query)
        || process.cmdline.to_ascii_lowercase().contains(query)
}

fn container_matches(container: &DockerContainer, query: &str) -> bool {
    container.name.to_ascii_lowercase().contains(query)
        || container.image.to_ascii_lowercase().contains(query)
        || container.status.to_ascii_lowercase().contains(query)
        || format_ports(container).contains(query)
}

fn volume_matches(volume: &DockerVolume, query: &str) -> bool {
    volume.name.to_ascii_lowercase().contains(query)
        || volume.driver.to_ascii_lowercase().contains(query)
        || volume
            .container_names
            .iter()
            .any(|name| name.to_ascii_lowercase().contains(query))
}

fn format_ports(container: &DockerContainer) -> String {
    if container.host_ports.is_empty() {
        return String::new();
    }

    container
        .host_ports
        .iter()
        .map(|port| port.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Protocol, SystemStats};
    use crate::tui::app::{InputMode, Snapshot};

    fn empty_stats() -> SystemStats {
        SystemStats {
            total_memory: 0,
            used_memory: 0,
            global_cpu_usage: 0.0,
            cpu_temp_c: None,
            gpu_temp_c: None,
        }
    }

    fn port(
        port: u16,
        address: &str,
        pid: Option<u32>,
        process_name: Option<&str>,
        container_name: Option<&str>,
    ) -> PortBinding {
        PortBinding {
            port,
            protocol: Protocol::Tcp,
            address: address.to_string(),
            pid,
            process_name: process_name.map(str::to_string),
            container_name: container_name.map(str::to_string),
            container_image: None,
        }
    }

    fn process(pid: u32, name: &str, cmdline: &str) -> DevProcess {
        DevProcess {
            pid,
            name: name.to_string(),
            cmdline: cmdline.to_string(),
            memory_bytes: 0,
            cpu_usage: 0.0,
            is_dev: true,
        }
    }

    fn container(name: &str, image: &str, status: &str, host_ports: Vec<u16>) -> DockerContainer {
        DockerContainer {
            id: format!("id-{name}"),
            name: name.to_string(),
            image: image.to_string(),
            status: status.to_string(),
            host_ports,
            cpu_percent: None,
            memory_bytes: None,
        }
    }

    fn app_with(tab: Tab, snapshot: Snapshot, query: &str) -> App {
        let mut app = App::new(crate::config::Config::default());
        app.tab = tab;
        app.snapshot = Some(snapshot);
        app.search_query = query.to_string();
        app.input_mode = InputMode::Normal;

        app
    }

    fn snapshot(
        ports: Vec<PortBinding>,
        processes: Vec<DevProcess>,
        containers: Vec<DockerContainer>,
    ) -> Snapshot {
        Snapshot {
            ports,
            processes,
            containers,
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        }
    }

    #[test]
    fn empty_query_return_no_matches() {
        let app = app_with(
            Tab::Ports,
            snapshot(
                vec![port(8080, "127.0.0.1", Some(1), Some("node"), None)],
                vec![],
                vec![],
            ),
            "   ",
        );
        assert!(find_matches(&app).is_empty());
    }

    #[test]
    fn no_snapshot_returns_no_matches() {
        let mut app = App::new(crate::config::Config::default());
        app.search_query = "8080".to_string();
        assert!(find_matches(&app).is_empty());
    }

    #[test]
    fn ports_match_by_port() {
        let app = app_with(
            Tab::Ports,
            snapshot(
                vec![
                    port(8080, "127.0.0.1", Some(1), Some("node"), None),
                    port(6379, "0.0.0.0", Some(2), None, Some("redis-dev")),
                ],
                vec![],
                vec![],
            ),
            "8080",
        );
        assert_eq!(find_matches(&app), vec![0]);
    }

    #[test]
    fn ports_match_by_process_owner() {
        let app = app_with(
            Tab::Ports,
            snapshot(
                vec![port(3000, "127.0.0.1", Some(9), Some("vite"), None)],
                vec![],
                vec![],
            ),
            "vite",
        );

        assert_eq!(find_matches(&app), vec![0]);
    }

    #[test]
    fn ports_match_docker_owner_lowercase() {
        let app = app_with(
            Tab::Ports,
            snapshot(
                vec![port(6379, "0.0.0.0", None, None, Some("redis-dev"))],
                vec![],
                vec![],
            ),
            "redis",
        );
        assert_eq!(find_matches(&app), vec![0])
    }

    #[test]
    fn processes_match_name_and_cmdline_case_insensitive() {
        let app = app_with(
            Tab::Processes,
            snapshot(
                vec![],
                vec![
                    process(10, "bash", "/bin/bash"),
                    process(20, "node", "node /app/Server.js"),
                ],
                vec![],
            ),
            "SERVER",
        );
        assert_eq!(find_matches(&app), vec![1]);
    }

    #[test]
    fn docker_match_by_name_images_status_port() {
        let app = app_with(
            Tab::Docker,
            snapshot(
                vec![],
                vec![],
                vec![
                    container("api", "node:20", "running", vec![8080]),
                    container("db", "postgres:16", "exited", vec![5432]),
                ],
            ),
            "5432",
        );

        assert_eq!(find_matches(&app), vec![1]);
    }

    #[test]
    fn only_active_tab_is_searched() {
        let app = app_with(
            Tab::Processes,
            snapshot(
                vec![port(8080, "127.0.0.1", Some(1), Some("node"), None)],
                vec![process(1, "cargo", "cargo run")],
                vec![],
            ),
            "8080",
        );

        assert_eq!(find_matches(&app), vec![] as Vec<usize>);
    }
}
