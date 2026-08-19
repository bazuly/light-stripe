use crate::config::Config;
use crate::tui::app::App;
use crate::tui::collector_worker::{self, FromWorker, ToWorker};
use crate::tui::{events, ui};
use anyhow::Result;
use crossterm::event::{self, Event};
use std::time::{Duration, Instant};

pub fn run(config: Config) -> Result<()> {
    let refresh_every = Duration::from_secs(config.refresh_secs.max(1));
    let worker = collector_worker::spawn(config.clone());

    let result: Result<()> = ratatui::run(|terminal| {
        let mut app = App::new(config);
        let mut last_refresh = Instant::now() - refresh_every;
        let mut refresh_in_flight = true;

        // Kick off the first snapshot in the background.
        let _ = worker.to_worker.send(ToWorker::Refresh);

        loop {
            if app.should_quit {
                let _ = worker.to_worker.send(ToWorker::Quit);
                break;
            }

            // Apply finished snapshots without blocking the UI.
            while let Ok(msg) = worker.from_worker.try_recv() {
                match msg {
                    FromWorker::Ready {
                        snapshot,
                        volume_warning,
                    } => {
                        app.apply_snapshot(snapshot, volume_warning);
                        refresh_in_flight = false;
                        last_refresh = Instant::now();
                    }
                    FromWorker::Failed(error) => {
                        app.last_error = Some(error);
                        refresh_in_flight = false;
                        last_refresh = Instant::now();
                    }
                }
            }

            // Request a refresh when due, but never stack concurrent collects.
            let due = last_refresh.elapsed() >= refresh_every;
            if (app.needs_refresh || due) && !refresh_in_flight {
                let _ = worker.to_worker.send(ToWorker::Refresh);
                refresh_in_flight = true;
                app.needs_refresh = false;
            }

            // Always draw (possibly stale) data while collect runs off-thread.
            terminal.draw(|frame| ui::draw(frame, &mut app))?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key_event) = event::read()? {
                    events::handle_key(&mut app, key_event);
                }
            }
        }

        Ok(())
    });

    result
}
