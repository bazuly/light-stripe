use crate::collectors::snapshot::collect_snapshot;
use crate::config::Config;
use crate::tui::app::Snapshot;

use std::sync::mpsc;
use std::thread;

/// Commands from interface (tui) to worker
/// We send commands from ui to worker thread
/// to work with containers with separate thread

pub enum ToWorker {
    Refresh,
    Quit,
    StopContainers { ids: Vec<String> },
    RestartContainers { ids: Vec<String> },
    RemoveContainers { ids: Vec<String> },
    RemoveVolumes { names: Vec<String> },
    KillProcesses { pids: Vec<u32> },
}

pub enum FromWorker {
    Ready {
        snapshot: Snapshot,
        volume_warning: Option<String>,
    },
    Failed(String),
    /// any action command result
    ActionDone {
        summary: String,
        refresh: bool,
    },
}

/// thread worker channels
pub struct WorkerHandle {
    pub to_worker: mpsc::Sender<ToWorker>,
    pub from_worker: mpsc::Receiver<FromWorker>,
}

pub fn spawn(config: Config) -> WorkerHandle {
    let (to_tx, to_rx) = mpsc::channel::<ToWorker>();
    let (from_tx, from_rx) = mpsc::channel::<FromWorker>();

    thread::spawn(move || {
        worker_loop(config, to_rx, from_tx);
    });

    WorkerHandle {
        to_worker: to_tx,
        from_worker: from_rx,
    }
}

fn worker_loop(config: Config, to_rx: mpsc::Receiver<ToWorker>, from_tx: mpsc::Sender<FromWorker>) {
    let docker_host = config.docker_host().map(str::to_string);
    let _rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    loop {
        let cmd = match to_rx.recv() {
            Ok(cmd) => cmd,
            Err(_) => break,
        };

        match cmd {
            ToWorker::Quit => break,

            ToWorker::Refresh => {
                // Drain the queue: coalesce extra Refresh, keep actions for later.
                // std::mpsc cannot put messages back, so we must not drop actions.
                let mut should_quit = false;
                let mut pending_actions: Vec<ToWorker> = Vec::new();

                while let Ok(extra) = to_rx.try_recv() {
                    match extra {
                        ToWorker::Quit => {
                            should_quit = true;
                            break;
                        }
                        ToWorker::Refresh => {}
                        action @ (ToWorker::StopContainers { .. }
                        | ToWorker::RestartContainers { .. }
                        | ToWorker::RemoveContainers { .. }
                        | ToWorker::RemoveVolumes { .. }
                        | ToWorker::KillProcesses { .. }) => {
                            pending_actions.push(action);
                        }
                    }
                }

                if should_quit {
                    break;
                }

                let reply = match collect_snapshot(&config) {
                    Ok((snapshot, volume_warning)) => FromWorker::Ready {
                        snapshot,
                        volume_warning,
                    },
                    Err(e) => FromWorker::Failed(e.to_string()),
                };
                if from_tx.send(reply).is_err() {
                    break;
                }

                for action in pending_actions {
                    if handle_action(&docker_host, &from_tx, action).is_err() {
                        return;
                    }
                }
            }

            action @ (ToWorker::StopContainers { .. }
            | ToWorker::RestartContainers { .. }
            | ToWorker::RemoveContainers { .. }
            | ToWorker::RemoveVolumes { .. }
            | ToWorker::KillProcesses { .. }) => {
                if handle_action(&docker_host, &from_tx, action).is_err() {
                    break;
                }
            }
        }
    }
}

fn handle_action(
    docker_host: &Option<String>,
    from_tx: &mpsc::Sender<FromWorker>,
    cmd: ToWorker,
) -> Result<(), mpsc::SendError<FromWorker>> {
    match cmd {
        ToWorker::StopContainers { ids } => {
            let summary = run_many(&ids, |id| {
                crate::actions::docker::stop_container(id, docker_host.as_deref())
            });
            send_done(from_tx, "stopped", summary)
        }
        ToWorker::RestartContainers { ids } => {
            let summary = run_many(&ids, |id| {
                crate::actions::docker::restart_container(id, docker_host.as_deref())
            });
            send_done(from_tx, "restarted", summary)
        }
        ToWorker::RemoveContainers { ids } => {
            let summary = run_many(&ids, |id| {
                crate::actions::docker::remove_container(id, docker_host.as_deref())
            });
            send_done(from_tx, "removed", summary)
        }
        ToWorker::RemoveVolumes { names } => {
            let summary = run_many(&names, |name| {
                crate::actions::docker::remove_volume(name, docker_host.as_deref(), false)
            });
            send_done(from_tx, "removed volumes", summary)
        }
        ToWorker::KillProcesses { pids } => {
            let summary = run_many(&pids, |pid| crate::actions::process::kill_process(*pid));
            send_done(from_tx, "killed", summary)
        }
        ToWorker::Refresh | ToWorker::Quit => Ok(()),
    }
}

struct BatchResult {
    ok: usize,
    total: usize,
    last_error: Option<String>,
}

/// Receives many objects (container ids, volume names, or pids),
/// runs an action for each, counts successes / last error.
fn run_many<T, F>(items: &[T], mut f: F) -> BatchResult
where
    F: FnMut(&T) -> anyhow::Result<()>,
{
    let mut ok = 0;
    let mut last_error = None;
    for item in items {
        match f(item) {
            Ok(()) => ok += 1,
            Err(e) => last_error = Some(e.to_string()),
        }
    }

    BatchResult {
        ok,
        total: items.len(),
        last_error,
    }
}

fn send_done(
    from_tx: &mpsc::Sender<FromWorker>,
    verb: &str,
    summary: BatchResult,
) -> Result<(), mpsc::SendError<FromWorker>> {
    from_tx.send(FromWorker::ActionDone {
        summary: format_summary(verb, &summary),
        refresh: true,
    })
}

fn format_summary(verb: &str, r: &BatchResult) -> String {
    match &r.last_error {
        Some(err) => format!("{verb} {}/{}: {err}", r.ok, r.total),
        None => format!("{verb} {}/{}", r.ok, r.total),
    }
}
