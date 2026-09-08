mod actions;
mod cli;
mod collectors;
mod config;
mod docker_client;
mod models;
mod output;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, OutputFormat};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = match &cli.config {
        Some(path) => config::load_from(path),
        None => config::load(),
    };

    match cli.command {
        Command::Tui => {
            tui::run(cfg)?;
        }

        Command::Ports(args) => {
            let mut bindings = collectors::ports::collect(args.port)?;

            if let Ok(containers) = collectors::docker::collect(cfg.docker_host()) {
                collectors::enrich::attach_docker(&mut bindings, &containers);
            }

            match args.format {
                OutputFormat::Table => output::table::print_ports(&bindings),
                OutputFormat::Json => output::json::print_ports(&bindings)?,
            }
        }

        Command::Ps(args) => {
            let processes = collectors::processes::collect_dev_processes(
                args.dev_only,
                &cfg.extra_dev_markers,
            )?;

            match args.format {
                OutputFormat::Table => output::table::print_processes(&processes),
                OutputFormat::Json => output::json::print_processes(&processes)?,
            }
        }
        Command::Stats(args) => {
            let stats = collectors::system::collect()?;
            match args.format {
                OutputFormat::Table => output::table::print_stats(&stats),
                OutputFormat::Json => output::json::print_stats(&stats)?,
            }
        }
        Command::Config => {
            config::print_effective(&cfg);
        }
    }

    Ok(())
}
