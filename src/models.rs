use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Serialize, Clone)]
pub struct PortBinding {
    pub port: u16,
    pub protocol: Protocol,
    pub address: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub container_name: Option<String>,
    pub container_image: Option<String>,
}

/// Local process row (dev tab and "all processes" tab share this type).
#[derive(Serialize, Clone)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
    pub memory_bytes: u64,
    pub cpu_usage: f32,
    pub is_dev: bool,
}

/// Back-compat alias used by CLI output helpers.
pub type DevProcess = Process;

#[derive(Serialize, Clone)]
pub struct SystemStats {
    pub total_memory: u64,
    pub used_memory: u64,
    pub global_cpu_usage: f32,
    pub cpu_temp_c: Option<f32>,
    pub gpu_temp_c: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub host_ports: Vec<u16>,
    pub cpu_percent: Option<f32>,
    pub memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DockerVolume {
    pub name: String,
    pub driver: String,
    pub size_bytes: Option<u64>,
    pub in_use: bool,
    /// Container names that mount this volume.
    pub container_names: Vec<String>,
}

pub struct Snapshot {
    pub ports: Vec<PortBinding>,
    pub dev_processes: Vec<Process>,
    pub regular_processes: Vec<Process>,
    pub containers: Vec<DockerContainer>,
    pub docker_error: Option<String>,
    pub volumes: Vec<DockerVolume>,
    pub stats: SystemStats,
}

#[allow(dead_code)]
impl Snapshot {
    pub fn empty(stats: SystemStats) -> Self {
        Self {
            ports: Vec::new(),
            dev_processes: Vec::new(),
            regular_processes: Vec::new(),
            containers: Vec::new(),
            docker_error: None,
            volumes: Vec::new(),
            stats,
        }
    }
}
