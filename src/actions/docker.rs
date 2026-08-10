use anyhow::{Context, Result};
use bollard::Docker;
use bollard::query_parameters::{
    RemoveContainerOptionsBuilder, RestartContainerOptionsBuilder, StopContainerOptionsBuilder,
};
use tokio::runtime::Runtime;

pub fn stop_container(id: &str, docker_host: Option<&str>) -> Result<()> {
    let id = id.to_string();
    with_docker(docker_host, |docker| async move {
        let options = StopContainerOptionsBuilder::default().build();
        docker
            .stop_container(&id, Some(options))
            .await
            .context("docker stop failed")
    })
}

pub fn restart_container(id: &str, docker_host: Option<&str>) -> Result<()> {
    let id = id.to_string();
    with_docker(docker_host, |docker| async move {
        let options = RestartContainerOptionsBuilder::default().build();
        docker
            .restart_container(&id, Some(options))
            .await
            .context("docker restart failed")
    })
}

pub fn remove_container(id: &str, docker_host: Option<&str>) -> Result<()> {
    let id = id.to_string();
    with_docker(docker_host, |docker| async move {
        let options = RemoveContainerOptionsBuilder::default().force(true).build();
        docker
            .remove_container(&id, Some(options))
            .await
            .context("docker container remove failed")
    })
}

fn with_docker<F, Fut>(docker_host: Option<&str>, action: F) -> Result<()>
where
    F: FnOnce(Docker) -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let runtime = Runtime::new().context("failed to create tokio runtime")?;
    runtime.block_on(async {
        let docker = crate::docker_client::connect(docker_host)?;
        action(docker).await
    })
}
