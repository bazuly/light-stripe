use crate::collectors::docker::collect as collect_docker;
use crate::collectors::{enrich, ports, processes, system, volumes};
use crate::config::Config;
use crate::models::Snapshot;

use anyhow::Result;

pub fn collect_snapshot(config: &Config) -> Result<(Snapshot, Option<String>)> {
    let mut ports = ports::collect(None)?;
    ports.retain(|b| !config.ignored_ports.contains(&b.port));
    let (dev_processes, regular_processes) = processes::collect_split(&config.extra_dev_markers)?;
    let stats = system::collect()?;
    let mut volume_warning: Option<String> = None;
    let (containers, docker_error) = match collect_docker(config.docker_host()) {
        Ok(containers) => {
            enrich::attach_docker(&mut ports, &containers);
            (containers, None) // return
        }
        Err(error) => (Vec::new(), Some(error.to_string())), // return
    };
    let volumes = if docker_error.is_some() {
        Vec::new()
    } else {
        match volumes::collect(config.docker_host()) {
            Ok(v) => v,
            Err(error) => {
                volume_warning = Some(format!("volumes unavailable: {error}"));
                Vec::new()
            }
        }
    };
    Ok((
        Snapshot {
            ports,
            dev_processes,
            regular_processes,
            containers,
            volumes,
            docker_error,
            stats,
        },
        volume_warning,
    ))
}
