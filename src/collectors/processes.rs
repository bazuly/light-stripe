use crate::collectors::dev_markers::DEV_MARKERS;
use crate::models::Process;
use anyhow::Result;
use std::ffi::OsString;
use std::thread;
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, ProcessRefreshKind, ProcessesToUpdate, System};

/// One sysinfo pass → (dev processes, non-dev processes).
pub fn collect_split(extra_dev_markers: &[String]) -> Result<(Vec<Process>, Vec<Process>)> {
    let mut system = System::new();
    system.refresh_all();

    thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu(),
    );

    let mut dev_processes = Vec::new();
    let mut regular_processes = Vec::new();

    for process in system.processes().values() {
        let name: String = process.name().to_string_lossy().into_owned();
        let mut cmdline = format_cmdline(process.cmd());
        if cmdline.is_empty() {
            cmdline = name.clone();
        }

        let is_dev = is_dev_process(&name, &cmdline, extra_dev_markers);
        let row = Process {
            pid: process.pid().as_u32(),
            name,
            cmdline,
            memory_bytes: process.memory(),
            cpu_usage: process.cpu_usage(),
            is_dev,
        };

        if is_dev {
            dev_processes.push(row);
        } else {
            regular_processes.push(row);
        }
    }

    dev_processes.sort_by(|left, right| right.memory_bytes.cmp(&left.memory_bytes));
    regular_processes.sort_by(|left, right| right.memory_bytes.cmp(&left.memory_bytes));
    Ok((dev_processes, regular_processes))
}

/// CLI `ps` / `ps -d`.
pub fn collect_dev_processes(dev_only: bool, extra_dev_markers: &[String]) -> Result<Vec<Process>> {
    let (mut dev, regular) = collect_split(extra_dev_markers)?;
    if !dev_only {
        dev.extend(regular);
        dev.sort_by(|left, right| right.memory_bytes.cmp(&left.memory_bytes));
    }
    Ok(dev)
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

    #[test]
    fn is_dev_is_case_insensitive() {
        assert!(is_dev_process("NODE", "NODE Server.JS", &[]));
        assert!(is_dev_process("Cargo", "CARGO RUN", &[]));
    }

    #[test]
    fn detects_path_style_rust_and_go_markers() {
        assert!(is_dev_process(
            "mybin",
            "/Users/me/proj/target/debug/mybin",
            &[]
        ));
        assert!(is_dev_process("go", "go run ./cmd/api", &[]));
    }

    #[test]
    fn docker_related_names_count_as_dev() {
        assert!(is_dev_process("com.docker.backend", "docker desktop", &[]));
        assert!(is_dev_process("compose", "docker compose up", &[]));
    }

    #[test]
    fn detects_editors_and_ides() {
        assert!(is_dev_process("zed", "/Applications/Zed.app/Contents/MacOS/zed", &[]));
        assert!(is_dev_process(
            "Cursor Helper",
            "/Applications/Cursor.app/Contents/Frameworks/Cursor Helper",
            &[]
        ));
        assert!(is_dev_process(
            "Code Helper",
            "Visual Studio Code Helper (Plugin)",
            &[]
        ));
        assert!(is_dev_process("idea", "IntelliJ IDEA", &[]));
        assert!(is_dev_process("nvim", "nvim src/main.rs", &[]));
        assert!(is_dev_process("Xcode", "Xcode", &[]));
    }
}
