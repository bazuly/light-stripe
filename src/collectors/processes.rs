use crate::models::{DevProcess, RegularProcess};
use anyhow::Result;
use std::ffi::OsString;
use std::thread;
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, ProcessRefreshKind, ProcessesToUpdate, System};

pub fn collect_dev_processes(
    dev_only: bool,
    extra_dev_markers: &[String],
) -> Result<Vec<DevProcess>> {
    let mut system = System::new();

    system.refresh_all();

    thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu(),
    );

    let mut dev_processes: Vec<DevProcess> = Vec::new();

    for process in system.processes().values() {
        let name: String = process.name().to_string_lossy().into_owned();

        let mut cmdline: String = format_cmdline(process.cmd());

        if cmdline.is_empty() {
            cmdline = name.clone();
        }

        let is_dev: bool = is_dev_process(&name, &cmdline, extra_dev_markers);

        if dev_only && !is_dev {
            continue;
        }

        let pid: u32 = process.pid().as_u32();
        let memory_bytes: u64 = process.memory();
        let cpu_usage: f32 = process.cpu_usage();

        dev_processes.push(DevProcess {
            pid,
            name,
            cmdline,
            memory_bytes,
            cpu_usage,
            is_dev,
        });
    }
    dev_processes.sort_by(|left, right| right.memory_bytes.cmp(&left.memory_bytes));
    Ok(dev_processes)
}

pub fn collect_regular_processes() -> Result<Vec<RegularProcess>> {
    let mut system = System::new();

    system.refresh_all();

    thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu(),
    );

    let mut regular_processes: Vec<RegularProcess> = Vec::new();

    for process in system.processes().values() {
        let name: String = process.name().to_string_lossy().into_owned();

        let mut cmdline: String = format_cmdline(process.cmd());

        if cmdline.is_empty() {
            cmdline = name.clone();
        }

        let pid: u32 = process.pid().as_u32();
        let memory_bytes: u64 = process.memory();
        let cpu_usage: f32 = process.cpu_usage();

        regular_processes.push(RegularProcess {
            pid,
            name,
            cmdline,
            memory_bytes,
            cpu_usage,
        });
    }
    regular_processes.sort_by(|left, right| right.memory_bytes.cmp(&left.memory_bytes));
    Ok(regular_processes)
}

///   ["node", "/path/vite"] → "node /path/vite"
fn format_cmdline(cmd_parts: &[OsString]) -> String {
    let cmd_pieces: Vec<String> = cmd_parts
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();

    cmd_pieces.join(" ")
}

fn is_dev_process(name: &str, cmdline: &str, extra_markers: &[String]) -> bool {
    const DEV_MARKERS: &[&str] = &[
        // JS
        "node",
        "npm",
        "pnpm",
        "yarn",
        "bun",
        "deno",
        "vite",
        "next",
        "nuxt",
        "nest",
        "webpack",
        "esbuild",
        "turbo",
        // Python
        "python",
        "uvicorn",
        "gunicorn",
        "django",
        "flask",
        "fastapi",
        "poetry",
        "celery",
        "manage.py",
        // Rust
        "cargo",
        "rustc",
        "target/debug",
        "target/release",
        // Go
        "go run",
        "air",
        // JVM
        "java",
        "gradle",
        "mvn",
        "maven",
        "spring",
        // .NET
        "dotnet",
        // Ruby / PHP
        "ruby",
        "rails",
        "puma",
        "php",
        "composer",
        "artisan",
        // DB / cache
        "postgres",
        "redis",
        "mongod",
        "mysql",
        "mariadb",
        "elasticsearch",
        "rabbitmq",
        "kafka",
        "minio",
        "memcached",
        // containers / k8s local
        "docker",
        "compose",
        "podman",
        "kubectl",
        "minikube",
        "kind",
        // local reverse proxy
        "nginx",
        "caddy",
        "traefik",
    ];

    let haystack: String = format!("{} {}", name, cmdline).to_lowercase();
    if DEV_MARKERS.iter().any(|marker| haystack.contains(marker)) {
        return true;
    }
    extra_markers
        .iter()
        .any(|marker| haystack.contains(&marker.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn detects_node_and_cargo_as_dev() {
        assert!(is_dev_process("node", "node server.js", &[]));
        assert!(is_dev_process("cargo", "cargo run", &[]));
        assert!(is_dev_process("python3", "uvicorn app:main", &[]));
    }
    #[test]
    fn rejects_plain_shell() {
        assert!(!is_dev_process("bash", "-bash", &[]));
        assert!(!is_dev_process("sleep", "sleep 10", &[]));
    }
    #[test]
    fn respects_extra_markers_from_config() {
        let extra = vec!["my-legacy-app".to_string()];
        assert!(is_dev_process("foo", "foo my-legacy-app", &extra));
        assert!(!is_dev_process("foo", "foo unrelated", &[]));
    }
    #[test]
    fn format_cmdline_joins_parts() {
        let parts = [OsString::from("node"), OsString::from("/app/vite")];
        assert_eq!(format_cmdline(&parts), "node /app/vite");
    }
    #[test]
    fn format_cmdline_empty() {
        assert_eq!(format_cmdline(&[]), "");
    }
}
