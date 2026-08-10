use crate::docker_client;
use crate::models::DockerContainer;
use anyhow::{Context, Result};
use bollard::Docker;
use bollard::models::ContainerSummary;
use bollard::plugin::{ContainerStatsResponse, ContainerSummaryStateEnum};
use bollard::query_parameters::{ListContainersOptionsBuilder, StatsOptionsBuilder};
use futures_util::StreamExt;
use futures_util::future::join_all;
use tokio::runtime::Runtime;

pub fn collect(docker_host: Option<&str>) -> Result<Vec<DockerContainer>> {
    let runtime = Runtime::new().context("failed to create tokio runtime")?;

    runtime.block_on(collect_async(docker_host))
}

async fn collect_async(docker_host: Option<&str>) -> Result<Vec<DockerContainer>> {
    let docker = docker_client::connect(docker_host)?;

    // collect all docker containers, analogy "docker ps -a"
    let options = ListContainersOptionsBuilder::default().all(true).build();
    let summaries = docker.list_containers(Some(options)).await?;

    let mut containers: Vec<DockerContainer> = summaries
        .iter()
        .map(|summary| DockerContainer {
            id: container_id(summary),
            name: container_name(summary),
            image: short_image(summary.image.as_deref().unwrap_or("unknown")),
            status: container_status(summary.state.as_ref()),
            host_ports: extract_host_ports(summary),
            cpu_percent: None,
            memory_bytes: None,
        })
        .collect();

    let stats_jobs = containers
        .iter()
        .enumerate()
        .filter_map(|(index, container)| {
            if container.status != "running" {
                return None;
            }

            let docker = docker.clone();
            let container_id = container.id.clone();

            Some(async move {
                let stats = fetch_stats(&docker, &container_id).await;
                // return
                (index, stats)
            })
        });

    for (index, stats) in join_all(stats_jobs).await {
        if let Some((cpu, mem)) = stats {
            containers[index].cpu_percent = Some(cpu);
            containers[index].memory_bytes = Some(mem);
        }
    }
    containers.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(containers)
}

async fn fetch_stats(docker: &Docker, container_id: &str) -> Option<(f32, u64)> {
    let options = StatsOptionsBuilder::default().stream(false).build();
    // docker data stream
    let mut stream = docker.stats(container_id, Some(options));

    let response = stream.next().await?.ok()?;
    let cpu = calc_cpu_percent(&response)?;
    let mem = response.memory_stats.as_ref()?.usage?;

    Some((cpu, mem))
}

fn calc_cpu_percent(stats: &ContainerStatsResponse) -> Option<f32> {
    let cpu = stats.cpu_stats.as_ref()?;
    let precpu = stats.precpu_stats.as_ref()?;

    let cpu_total = cpu.cpu_usage.as_ref()?.total_usage?;
    let precpu_total = precpu.cpu_usage.as_ref()?.total_usage?;

    let system = cpu.system_cpu_usage?;
    let presystem = precpu.system_cpu_usage?;

    let cpu_delta = cpu_total as f64 - precpu_total as f64;
    let system_delta = system as f64 - presystem as f64;

    if cpu_delta <= 0.0 || system_delta <= 0.0 {
        return Some(0.0);
    }

    let online_cpus = cpu
        .online_cpus
        .map(|n| n as f64)
        .or_else(|| {
            cpu.cpu_usage
                .as_ref()?
                .percpu_usage
                .as_ref()
                .map(|cores| cores.len() as f64)
        })
        .unwrap_or(1.0);
    Some(((cpu_delta / system_delta) * online_cpus * 100.0) as f32)
}

fn extract_host_ports(summary: &ContainerSummary) -> Vec<u16> {
    let mut ports: Vec<u16> = Vec::new();

    let Some(port_list) = &summary.ports else {
        return ports;
    };

    for port in port_list {
        if let Some(public_port) = port.public_port {
            ports.push(public_port);
        }
    }

    ports
}

fn short_image(image: &str) -> String {
    image.rsplit('/').next().unwrap_or(image).to_string()
}

// search with container id
fn container_id(summary: &ContainerSummary) -> String {
    summary
        .id
        .clone()
        // edge case, better catch container_name instead of None
        .unwrap_or_else(|| container_name(summary))
}

// search with container name
fn container_name(summary: &ContainerSummary) -> String {
    summary
        .names
        .as_ref()
        .and_then(|names| names.first())
        .map(|name| name.trim_start_matches("/").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn container_status(state: Option<&ContainerSummaryStateEnum>) -> String {
    use ContainerSummaryStateEnum::*;
    let label = match state {
        Some(RUNNING) => "running",
        Some(EXITED) => "exited",
        Some(CREATED) => "created",
        Some(PAUSED) => "paused",
        Some(RESTARTING) => "restarting",
        Some(REMOVING) => "removing",
        Some(DEAD) => "dead",
        Some(EMPTY) | None => "unknown",
        Some(STOPPING) => "stopping",
    };

    label.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::models::{
        ContainerCpuStats, ContainerCpuUsage, ContainerStatsResponse, ContainerSummary, PortSummary,
    };
    use bollard::plugin::ContainerSummaryStateEnum;

    fn summary_with(
        id: Option<&str>,
        names: Option<Vec<&str>>,
        ports: Option<Vec<PortSummary>>,
    ) -> ContainerSummary {
        ContainerSummary {
            id: id.map(str::to_string),
            names: names.map(|ns| ns.into_iter().map(str::to_string).collect()),
            ports,
            ..Default::default()
        }
    }

    fn port(public: Option<u16>, private: u16) -> PortSummary {
        PortSummary {
            public_port: public,
            private_port: private,
            ..Default::default()
        }
    }

    fn stats_fixture(
        cpu_total: u64,
        precpu_total: u64,
        system: u64,
        presystem: u64,
        online_cpus: Option<u32>,
        percpu_usage: Option<Vec<u64>>,
    ) -> ContainerStatsResponse {
        ContainerStatsResponse {
            cpu_stats: Some(ContainerCpuStats {
                cpu_usage: Some(ContainerCpuUsage {
                    total_usage: Some(cpu_total),
                    percpu_usage,
                    ..Default::default()
                }),
                system_cpu_usage: Some(system),
                online_cpus,
                ..Default::default()
            }),
            precpu_stats: Some(ContainerCpuStats {
                cpu_usage: Some(ContainerCpuUsage {
                    total_usage: Some(precpu_total),
                    ..Default::default()
                }),
                system_cpu_usage: Some(presystem),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    #[test]
    fn short_image_strips_registry_path() {
        assert_eq!(
            short_image("docker.io/library/nginx:latest"),
            "nginx:latest"
        );
        assert_eq!(short_image("nginx:alpine"), "nginx:alpine");
        assert_eq!(short_image(""), "");
    }

    #[test]
    fn container_name_trims_leading_slash() {
        let s = summary_with(None, Some(vec!["/my-app"]), None);
        assert_eq!(container_name(&s), "my-app");
    }

    #[test]
    fn container_name_unknown_when_missing() {
        let missing = summary_with(None, None, None);
        assert_eq!(container_name(&missing), "unknown")
    }

    #[test]
    fn container_id_uses_id_when_present() {
        let s = summary_with(Some("container_id"), Some(vec!["container_name"]), None);
        assert_eq!(container_id(&s), "container_id")
    }

    #[test]
    fn container_id_falls_back_to_name() {
        let s = summary_with(None, Some(vec!["container_name"]), None);
        assert_eq!(container_id(&s), "container_name")
    }

    #[test]
    fn container_status_maps_known_states() {
        use ContainerSummaryStateEnum::*;
        assert_eq!(container_status(Some(&RUNNING)), "running");
        assert_eq!(container_status(Some(&EXITED)), "exited");
        assert_eq!(container_status(Some(&CREATED)), "created");
        assert_eq!(container_status(Some(&PAUSED)), "paused");
        assert_eq!(container_status(Some(&RESTARTING)), "restarting");
        assert_eq!(container_status(Some(&REMOVING)), "removing");
        assert_eq!(container_status(Some(&DEAD)), "dead");
        assert_eq!(container_status(Some(&STOPPING)), "stopping");
    }

    #[test]
    fn container_status_unknown_for_none_and_empty() {
        use ContainerSummaryStateEnum::EMPTY;
        assert_eq!(container_status(None), "unknown");
        assert_eq!(container_status(Some(&EMPTY)), "unknown");
    }

    #[test]
    fn extract_host_ports_empty_when_no_ports_field() {
        let s = summary_with(None, None, None);
        assert!(extract_host_ports(&s).is_empty())
    }

    #[test]
    fn extract_host_ports_keeps_only_public_ports() {
        let s = summary_with(
            None,
            None,
            Some(vec![port(None, 8080), port(Some(3000), 3000)]),
        );
        assert_eq!(extract_host_ports(&s), vec![3000]);
    }

    #[test]
    fn calc_cpu_percent_returns_none_when_incomplete() {
        assert!(calc_cpu_percent(&ContainerStatsResponse::default()).is_none());
    }

    #[test]
    fn calc_cpu_percent_zero_on_non_positive_delta() {
        // cpu delta 0, cpu total (100) - precpu_total (100) = 0
        let stats = stats_fixture(100, 100, 2_000, 1_000, Some(1), None);
        assert_eq!(calc_cpu_percent(&stats), Some(0.0));
    }

    #[test]
    fn calc_cpu_percent_computes_with_online_cpus() {
        // (100_000 / 1_000_000) * 2 * 100 = 20.0
        let stats = stats_fixture(200_000, 100_000, 2_000_000, 1_000_000, Some(2), None);
        assert_eq!(calc_cpu_percent(&stats), Some(20.0));
    }

    #[test]
    fn calc_cpu_percent_uses_percpu_len_when_online_cpus_missing() {
        // multiplier = 4 cores => 40.0
        let stats = stats_fixture(
            200_000,
            100_000,
            2_000_000,
            1_000_000,
            None,
            Some(vec![1, 2, 3, 4]),
        );
        assert_eq!(calc_cpu_percent(&stats), Some(40.0));
    }
}
