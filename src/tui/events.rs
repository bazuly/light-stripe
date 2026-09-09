use crate::tui::{
    app::{App, InputMode, Tab},
    state::nav::{next_tab, tab_at_digit},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match &app.input_mode {
        InputMode::ConfirmDockerRemove { .. }
        | InputMode::ConfirmProcessRemove { .. }
        | InputMode::ConfirmVolumeRemove { .. } => {
            handle_confirm_key(app, key);
            return;
        }
        InputMode::Search => {
            handle_search_key(app, key);
            return;
        }
        InputMode::Normal => {}
    }
    handle_normal_key(app, key);
}

fn handle_confirm_key(app: &mut App, key: KeyEvent) {
    let tab = app.nav.tab;
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => match &app.input_mode {
            InputMode::ConfirmDockerRemove { .. } => app.confirm_docker_remove(),
            InputMode::ConfirmProcessRemove { .. } => app.confirm_kill_selected_process(tab),
            InputMode::ConfirmVolumeRemove { .. } => app.confirm_volume_remove(),
            _ => {}
        },
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.cancel_pending_action();
        }
        _ => {}
    }
}

fn handle_search_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.cancel_search(),
        KeyCode::Enter => {
            app.apply_search(0);
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => app.search.pop_char(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.search.push_char(ch);
        }
        _ => {}
    }
}

fn handle_normal_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,
        KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('/') => app.start_search(),
        KeyCode::Char('n') => app.apply_search(1),
        KeyCode::Char('N') => app.apply_search(-1),
        KeyCode::Char('r') | KeyCode::Char('R') => app.needs_refresh = true,

        KeyCode::Char(c @ '1'..='9') => {
            if let Some(tab) = tab_at_digit(&app.config, c as u8 - b'0') {
                app.set_tab(tab);
            }
        }

        KeyCode::Tab => {
            let next = next_tab(&app.config, app.nav.tab);
            app.set_tab(next);
        }

        KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
        KeyCode::PageUp => app.move_selection(-20),
        KeyCode::PageDown => app.move_selection(20),

        KeyCode::Home => {
            if app.active_list_len() > 0 {
                app.nav.select_row(0);
            }
        }

        KeyCode::End => {
            let len = app.active_list_len();
            if len > 0 {
                app.nav.select_row(len - 1);
            }
        }

        KeyCode::Char('x') | KeyCode::Char('X')
            if app.nav.tab == Tab::DevProcesses || app.nav.tab == Tab::RegularProcesses =>
        {
            app.request_kill_selected_process();
        }

        KeyCode::Char('s') if app.nav.tab == Tab::Docker => app.stop_selected_container(),
        KeyCode::Char('S') if app.nav.tab == Tab::Docker => app.restart_selected_container(),
        KeyCode::Char('d') | KeyCode::Char('D') if app.nav.tab == Tab::Docker => {
            app.request_remove_selected_container();
        }
        KeyCode::Char('d') | KeyCode::Char('D') if app.nav.tab == Tab::Volumes => {
            app.request_remove_selected_volumes();
        }

        KeyCode::Enter | KeyCode::Char('g') if app.nav.tab == Tab::Ports => {
            app.jump_from_selected_port();
        }
        KeyCode::Enter | KeyCode::Char('g') if app.nav.tab == Tab::Volumes => {
            app.jump_from_selected_volume();
        }

        KeyCode::Char(' ') => app.toggle_mark_current(),
        KeyCode::Char('a') => app.mark_all(),
        KeyCode::Char('A') => app.unmark_all(),

        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::models::{Process, Snapshot, SystemStats};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn empty_stats() -> SystemStats {
        SystemStats {
            total_memory: 0,
            used_memory: 0,
            global_cpu_usage: 0.0,
            cpu_temp_c: None,
            gpu_temp_c: None,
        }
    }

    fn process(pid: u32, name: &str) -> Process {
        Process {
            pid,
            name: name.to_string(),
            cmdline: name.to_string(),
            memory_bytes: 0,
            cpu_usage: 0.0,
            is_dev: false,
        }
    }

    fn app_with_processes() -> App {
        let mut app = App::new(Config::default());
        app.snapshot = Some(Snapshot {
            ports: vec![],
            dev_processes: vec![process(1, "node")],
            regular_processes: vec![process(2, "zsh"), process(3, "sleep")],
            containers: vec![],
            docker_error: None,
            volumes: vec![],
            stats: empty_stats(),
        });
        app
    }

    #[test]
    fn digit_keys_select_all_five_tabs() {
        let mut app = app_with_processes();
        handle_key(&mut app, key(KeyCode::Char('3')));
        assert_eq!(app.nav.tab, Tab::RegularProcesses);
        handle_key(&mut app, key(KeyCode::Char('2')));
        assert_eq!(app.nav.tab, Tab::DevProcesses);
        handle_key(&mut app, key(KeyCode::Char('5')));
        assert_eq!(app.nav.tab, Tab::Volumes);
        handle_key(&mut app, key(KeyCode::Char('1')));
        assert_eq!(app.nav.tab, Tab::Ports);
        handle_key(&mut app, key(KeyCode::Char('4')));
        assert_eq!(app.nav.tab, Tab::Docker);
    }

    #[test]
    fn tab_cycles_through_regular_processes() {
        let mut app = app_with_processes();
        assert_eq!(app.nav.tab, Tab::Ports);

        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.nav.tab, Tab::DevProcesses);
        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.nav.tab, Tab::RegularProcesses);
        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.nav.tab, Tab::Docker);
        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.nav.tab, Tab::Volumes);
        handle_key(&mut app, key(KeyCode::Tab));
        assert_eq!(app.nav.tab, Tab::Ports);
    }

    #[test]
    fn x_requests_kill_on_regular_processes_tab() {
        let mut app = app_with_processes();
        app.nav.tab = Tab::RegularProcesses;
        app.nav.select_row(1);

        handle_key(&mut app, key(KeyCode::Char('x')));

        match &app.input_mode {
            InputMode::ConfirmProcessRemove { targets } => {
                assert_eq!(targets, &[(3, "sleep".to_string())]);
            }
            other => panic!("expected confirm kill, got {other:?}"),
        }
    }

    #[test]
    fn x_ignored_on_ports_tab() {
        let mut app = app_with_processes();
        app.nav.tab = Tab::Ports;
        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(app.input_mode, InputMode::Normal);
    }
}
