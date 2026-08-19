use crate::collectors::snapshot::collect_snapshot;
use crate::config::Config;
use crate::tui::app::Snapshot;

use std::sync::mpsc;
use std::thread;

/// Commands from interface (tui) to worker
pub enum ToWorker {
    /// collect fresh snapshot
    Refresh,
    /// TUI exit
    Quit,
}

/// Answer from worket to interface (tui)
pub enum FromWorker {
    /// Collect successfully -> apply_snapshot()
    Ready {
        snapshot: Snapshot,
        volume_warning: Option<String>,
    },
    Failed(String),
}

// thread worker channels
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
    loop {
        let cmd = match to_rx.recv() {
            Ok(cmd) => cmd,
            Err(_) => break,
        };

        match cmd {
            ToWorker::Quit => break,

            ToWorker::Refresh => {
                let mut should_quit = false;
                while let Ok(extra) = to_rx.try_recv() {
                    match extra {
                        ToWorker::Quit => {
                            should_quit = true;
                            break;
                        }
                        ToWorker::Refresh => {}
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
                    Err(error) => FromWorker::Failed(error.to_string()),
                };

                if from_tx.send(reply).is_err() {
                    break;
                }
            }
        }
    }
}
