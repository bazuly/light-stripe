use crate::tui::collector_worker::ToWorker;
use std::sync::mpsc;

#[derive(Debug, Default)]
pub struct WorkerClient {
    tx: Option<mpsc::Sender<ToWorker>>,
    pub in_flight: bool,
}

impl WorkerClient {
    pub fn bind(&mut self, tx: mpsc::Sender<ToWorker>) {
        self.tx = Some(tx);
    }

    pub fn enqueue(&mut self, cmd: ToWorker) -> Result<(), &'static str> {
        if self.in_flight {
            return Err("action already running");
        }
        let Some(tx) = &self.tx else {
            return Err("worker not connected");
        };
        tx.send(cmd).map_err(|_| "worker died")?;
        self.in_flight = true;
        Ok(())
    }

    pub fn on_done(&mut self) {
        self.in_flight = false;
    }
}
