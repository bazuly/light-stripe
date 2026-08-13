use crate::docker_client;
use crate::models::DockerVolume;
use anyhow::{Context, Result};
use bollard::models::Volume;
use bollard::query_parameters::{DataUsageOptions, ListContainersOptionsBuilder};
use std::collections::HashMap;
use tokio::runtime::Runtime;

pub fn collect(docker_host: Option<&str>) -> Result<Vec<DockerVolume>> {
    let runtime = Runtime::new().context("failed to create tokio runtime")?;
    runtime.block_on(collect_async(docker_host))
}

async fn collect_async(docker_host: Option<&str>) -> Result<Vec<DockerVolume>> {
    let docker = docker_client::connect(docker_host)?;

    let listed = docker
        .list_volumes(None::<bollard::query_parameters::ListVolumesOptions>)
        .await
        .context("docker volume ls failed")?;
    let volume_list = listed.volumes.unwrap_or_default();

    // Sizes come from `docker system df` (soft-fail if unavailable).
    let mut sizes: HashMap<String, u64> = HashMap::new();
    if let Ok(df) = docker.df(None::<DataUsageOptions>).await {
        if let Some(usage) = df.volume_usage {
            for item in usage.items.unwrap_or_default() {
                if let Ok(volume) = serde_json::from_value::<Volume>(item) {
                    if let Some(usage_data) = volume.usage_data {
                        if usage_data.size >= 0 {
                            sizes.insert(volume.name, usage_data.size as u64);
                        }
                    }
                }
            }
        }
    }

    let usage = volume_usage_map(&docker).await.unwrap_or_default();

    let mut out: Vec<DockerVolume> = volume_list
        .into_iter()
        .map(|volume| {
            let name = volume.name;
            let driver = volume.driver;
            let size_bytes = sizes.get(&name).copied().or_else(|| {
                volume
                    .usage_data
                    .and_then(|ud| (ud.size >= 0).then_some(ud.size as u64))
            });
            let container_names = usage.get(&name).cloned().unwrap_or_default();
            let in_use = !container_names.is_empty();
            DockerVolume {
                name,
                driver,
                size_bytes,
                in_use,
                container_names,
            }
        })
        .collect();

    out.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(out)
}

async fn volume_usage_map(docker: &bollard::Docker) -> Result<HashMap<String, Vec<String>>> {
    let options = ListContainersOptionsBuilder::default().all(true).build();
    let containers = docker.list_containers(Some(options)).await?;

    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    for container in containers {
        let container_name = container
            .names
            .as_ref()
            .and_then(|names| names.first())
            .map(|name| name.trim_start_matches('/').to_string())
            .unwrap_or_else(|| "unknown".to_string());

        for mount in container.mounts.unwrap_or_default() {
            let is_volume = mount
                .typ
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case("volume"))
                .unwrap_or(false);
            if !is_volume {
                continue;
            }
            let Some(volume_name) = mount.name else {
                continue;
            };
            map.entry(volume_name)
                .or_default()
                .push(container_name.clone());
        }
    }

    for names in map.values_mut() {
        names.sort();
        names.dedup();
    }

    Ok(map)
}
