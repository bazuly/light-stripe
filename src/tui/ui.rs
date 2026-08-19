use crate::models::Protocol;
use crate::tui::app::{App, InputMode, Tab};

use crate::output::table::format_port_owner;
use crate::output::table::truncate_text;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Row, Table};

const BYTES_IN_GB: f64 = 1024.0 * 1024.0 * 1024.0;
const BYTES_IN_MB: f64 = 1024.0 * 1024.0;
const MAX_CMDLINE_LEN: usize = 60;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header: RAM/CPU
            Constraint::Length(1), // tabs
            Constraint::Min(0),    // main table
            Constraint::Length(1), // footer: hotkeys
        ])
        .split(frame.area());
    draw_header(frame, chunks[0], app);
    draw_tabs(frame, chunks[1], app);
    draw_main(frame, chunks[2], app);
    draw_footer(frame, chunks[3], app);
}

fn viewport_rows(area: Rect) -> usize {
    area.height.saturating_sub(4) as usize
}

fn draw_header(frame: &mut Frame, area: Rect, app: &mut App) {
    let text = if let Some(error) = &app.last_error {
        format!("Light Stripe | ERROR: {error}")
    } else if let Some(snapshot) = &app.snapshot {
        let used_gb = bytes_to_gb(snapshot.stats.used_memory);
        let total_gb = bytes_to_gb(snapshot.stats.total_memory);
        let cpu = snapshot.stats.global_cpu_usage;
        let cpu_temp = match snapshot.stats.cpu_temp_c {
            Some(temp) => format!(" {temp:.0}°C"),
            None => String::new(),
        };
        format!("Light Stripe | RAM {used_gb:.1}/{total_gb:.1} GB | CPU {cpu:.1}%{cpu_temp}")
    } else {
        "Light Stripe | Loading...".to_string()
    };
    let widget = Paragraph::new(text).block(Block::bordered().title(" LightStripe "));
    frame.render_widget(widget, area);
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &mut App) {
    let ports_label = if app.tab == Tab::Ports {
        "▶ 1:Ports"
    } else {
        "  1:Ports"
    };
    let processes_label = if app.tab == Tab::Processes {
        "▶ 2:DEV Processes"
    } else {
        "  2:DEV Processes"
    };

    let docker_label = if app.tab == Tab::Docker {
        "▶ 3:Docker"
    } else {
        "  3:Docker"
    };

    let volumes_label = if app.tab == Tab::Volumes {
        "▶ 4:Volumes"
    } else {
        "  4:Volumes"
    };

    let active = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let inactive = Style::new().fg(Color::DarkGray);

    let line = Line::from(vec![
        Span::styled(
            ports_label,
            if app.tab == Tab::Ports {
                active
            } else {
                inactive
            },
        ),
        Span::raw("   "),
        Span::styled(
            processes_label,
            if app.tab == Tab::Processes {
                active
            } else {
                inactive
            },
        ),
        Span::raw("   "),
        Span::styled(
            docker_label,
            if app.tab == Tab::Docker {
                active
            } else {
                inactive
            },
        ),
        Span::raw("   "),
        Span::styled(
            volumes_label,
            if app.tab == Tab::Volumes {
                active
            } else {
                inactive
            },
        ),
    ]);

    let widget = Paragraph::new(line);
    frame.render_widget(widget, area);
}

fn draw_main(frame: &mut Frame, area: Rect, app: &mut App) {
    match app.tab {
        Tab::Ports => draw_ports_table(frame, area, app),
        Tab::Processes => draw_processes_table(frame, area, app),
        Tab::Docker => draw_docker_table(frame, area, app),
        Tab::Volumes => draw_volumes_table(frame, area, app),
    }
}

fn draw_ports_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let visible = viewport_rows(area);
    app.ensure_visible(visible);

    let Some(snapshot) = &app.snapshot else {
        let widget = Paragraph::new("Loading ports...").block(Block::bordered().title("Ports"));
        frame.render_widget(widget, area);
        return;
    };
    if snapshot.ports.is_empty() {
        let widget =
            Paragraph::new("No listening ports found.").block(Block::bordered().title("Ports"));
        frame.render_widget(widget, area);
        return;
    }

    let header = Row::new(vec!["PORT", "PROTO", "ADDRESS", "PID", "OWNER"])
        .style(Style::new().add_modifier(Modifier::BOLD))
        .bottom_margin(1);
    let rows: Vec<Row> = snapshot
        .ports
        .iter()
        .map(|binding| {
            Row::new(vec![
                binding.port.to_string(),
                format_protocol(binding.protocol),
                binding.address.clone(),
                format_pid(binding.pid),
                format_port_owner(binding),
            ])
        })
        .collect();

    let selected_row = app.selected_row;
    let total = snapshot.ports.len();

    let widget = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(Block::bordered().title(format!("Ports [{}/{}]", selected_row + 1, total)))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");
    frame.render_stateful_widget(widget, area, &mut app.table_state);
}

fn draw_docker_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let visible = viewport_rows(area);
    app.ensure_visible(visible);

    let Some(snapshot) = &app.snapshot else {
        let widget =
            Paragraph::new("Loading containers...").block(Block::bordered().title("Docker"));
        frame.render_widget(widget, area);
        return;
    };

    if let Some(error) = &snapshot.docker_error {
        let widget = Paragraph::new(format!("Docker unavailable: {error}"))
            .block(Block::bordered().title("Docker"));
        frame.render_widget(widget, area);
        return;
    }

    if snapshot.containers.is_empty() {
        let widget =
            Paragraph::new("No containers found.").block(Block::bordered().title("Docker"));
        frame.render_widget(widget, area);
        return;
    }

    let header = Row::new(vec!["NAME", "IMAGE", "PORTS", "CPU", "MEM", "STATUS"])
        .style(Style::new().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = snapshot
        .containers
        .iter()
        .map(|container| {
            let mark = if app.marked_container_ids.contains(&container.id) {
                "●"
            } else {
                "○"
            };
            Row::new(vec![
                format!("{mark} {}", container.name),
                container.image.clone(),
                format_host_ports(&container.host_ports),
                format_optional_cpu(container.cpu_percent),
                format_optional_memory(container.memory_bytes),
                container.status.clone(),
            ])
        })
        .collect();
    let selected_row = app.selected_row;
    let total = snapshot.containers.len();
    let widget = Table::new(
        rows,
        [
            Constraint::Min(14),
            Constraint::Min(12),
            Constraint::Min(10),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(Block::bordered().title(format!("Docker [{}/{}]", selected_row + 1, total)))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");
    frame.render_stateful_widget(widget, area, &mut app.table_state);
}

fn draw_processes_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(snapshot) = &app.snapshot else {
        let widget =
            Paragraph::new("Loading processes...").block(Block::bordered().title("DEV Processes"));
        frame.render_widget(widget, area);
        return;
    };

    if snapshot.processes.is_empty() {
        let widget = Paragraph::new("No dev processes found.")
            .block(Block::bordered().title("DEV Processes"));
        frame.render_widget(widget, area);
        return;
    }

    let header = Row::new(vec!["PID", "CPU", "MEM", "NAME", "CMD"])
        .style(Style::new().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = snapshot
        .processes
        .iter()
        .map(|process| {
            let mark = if app.marked_pids.contains(&process.pid) {
                "●"
            } else {
                "○"
            };
            Row::new(vec![
                process.pid.to_string(),
                format_cpu(process.cpu_usage),
                format_memory_mb(process.memory_bytes),
                format!("{mark} {}", process.name),
                truncate_text(&process.cmdline, MAX_CMDLINE_LEN),
            ])
        })
        .collect();

    let selected_row = app.selected_row;
    let total = snapshot.processes.len();

    let widget = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(Block::bordered().title(format!("DEV Processes [{}/{}]", selected_row + 1, total)))
    .row_highlight_style(
        Style::new()
            .bg(Color::LightBlue)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");
    frame.render_stateful_widget(widget, area, &mut app.table_state);
}

fn draw_volumes_table(frame: &mut Frame, area: Rect, app: &mut App) {
    let visible = viewport_rows(area);
    app.ensure_visible(visible);

    let Some(snapshot) = &app.snapshot else {
        let widget = Paragraph::new("Loading volumes...").block(Block::bordered().title("Volumes"));
        frame.render_widget(widget, area);
        return;
    };

    if let Some(error) = &snapshot.docker_error {
        let widget = Paragraph::new(format!("Docker unavailable: {error}"))
            .block(Block::bordered().title("Volumes"));
        frame.render_widget(widget, area);
        return;
    }

    if snapshot.volumes.is_empty() {
        let widget = Paragraph::new("No volumes found.").block(Block::bordered().title("Volumes"));
        frame.render_widget(widget, area);
        return;
    }

    let header = Row::new(vec!["NAME", "DRIVER", "SIZE", "IN USE", "CONTAINERS"])
        .style(Style::new().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = snapshot
        .volumes
        .iter()
        .map(|volume| {
            let mark = if app.marked_volume_names.contains(&volume.name) {
                "●"
            } else {
                "○"
            };
            Row::new(vec![
                format!("{mark} {}", volume.name),
                volume.driver.clone(),
                format_optional_memory(volume.size_bytes),
                if volume.in_use {
                    "yes".to_string()
                } else {
                    "no".to_string()
                },
                if volume.container_names.is_empty() {
                    "-".to_string()
                } else {
                    volume.container_names.join(", ")
                },
            ])
        })
        .collect();

    let selected_row = app.selected_row;
    let total = snapshot.volumes.len();
    let widget = Table::new(
        rows,
        [
            Constraint::Min(16),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Min(16),
        ],
    )
    .header(header)
    .block(Block::bordered().title(format!("Volumes [{}/{}]", selected_row + 1, total)))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");
    frame.render_stateful_widget(widget, area, &mut app.table_state);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let text = if let InputMode::ConfirmDockerRemove { targets } = &app.input_mode {
        let names: Vec<&str> = targets.iter().map(|(_, name)| name.as_str()).collect();
        if names.len() == 1 {
            format!("Remove {}? [y/N]", names[0])
        } else {
            format!("Remove {} containers? [y/N]", names.len())
        }
    } else if let InputMode::ConfirmProcessRemove { targets } = &app.input_mode {
        if targets.len() == 1 {
            let (pid, name) = &targets[0];
            format!("Kill {name} (pid {pid})? [y/N]")
        } else {
            format!("Kill {} processes? [y/N]", targets.len())
        }
    } else if let InputMode::ConfirmVolumeRemove { targets } = &app.input_mode {
        if targets.len() == 1 {
            format!("Delete volume {}? [y/N]", targets[0])
        } else {
            format!("Delete {} volumes? [y/N]", targets.len())
        }
    } else if app.input_mode == InputMode::Search {
        format!("search: {}_", app.search_query)
    } else if let Some(status) = &app.status_message {
        format!("{status} | {}", footer_hints(app))
    } else if let Some(search) = app.select_search_status() {
        format!("{search} | {}", footer_hints(app))
    } else {
        footer_hints(app)
    };

    let widget = Paragraph::new(text);
    frame.render_widget(widget, area);
}

fn footer_hints(app: &App) -> String {
    match app.tab {
        Tab::Ports => "q: quit | r: refresh | /: search | Enter: jump | 1-4: tabs".to_string(),
        Tab::Processes => {
            let prefix = if app.marked_pids.is_empty() {
                String::new()
            } else {
                format!("{} selected processes | ", app.marked_pids.len())
            };
            format!("{prefix}Space: mark | a/A: all | x: kill | q: quit | /: search | 1-4: tabs")
        }
        Tab::Docker => {
            let prefix = if app.marked_container_ids.is_empty() {
                String::new()
            } else {
                format!("{} selected containers | ", app.marked_container_ids.len())
            };
            format!(
                "{prefix}Space: mark | a/A: all | s: stop | S: restart | d: remove | q: quit | 1-4: tabs"
            )
        }
        Tab::Volumes => {
            let prefix = if app.marked_volume_names.is_empty() {
                String::new()
            } else {
                format!("{} selected volumes | ", app.marked_volume_names.len())
            };
            format!(
                "{prefix}Space: mark | a/A: all | d: delete | Enter: jump | q: quit | 1-4: tabs"
            )
        }
    }
}
// formatting only for representation
fn bytes_to_gb(bytes: u64) -> f64 {
    bytes as f64 / BYTES_IN_GB
}

fn format_protocol(protocol: Protocol) -> String {
    match protocol {
        Protocol::Tcp => "tcp".to_string(),
        Protocol::Udp => "udp".to_string(),
    }
}

fn format_memory_mb(bytes: u64) -> String {
    let megabytes = bytes as f64 / BYTES_IN_MB;
    format!("{megabytes:.1} MB")
}

fn format_cpu(cpu_usage: f32) -> String {
    format!("{cpu_usage:.1} %")
}

fn format_pid(pid: Option<u32>) -> String {
    match pid {
        Some(value) => value.to_string(),
        None => "-".to_string(),
    }
}

fn format_host_ports(ports: &[u16]) -> String {
    if ports.is_empty() {
        return "-".to_string();
    }
    ports
        .iter()
        .map(|port| port.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_optional_cpu(cpu: Option<f32>) -> String {
    match cpu {
        Some(value) => format!("{value:.1}%"),
        None => "-".to_string(),
    }
}

fn format_optional_memory(bytes: Option<u64>) -> String {
    match bytes {
        Some(value) => format_memory_mb(value),
        None => "-".to_string(),
    }
}
