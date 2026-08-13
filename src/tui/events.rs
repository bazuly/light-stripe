use crate::tui::app::{App, InputMode, Tab};
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
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => match &app.input_mode {
            InputMode::ConfirmDockerRemove { .. } => app.confirm_docker_remove(),
            InputMode::ConfirmProcessRemove { .. } => app.confirm_kill_selected_process(),
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
        KeyCode::Backspace => app.pop_search_char(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.push_search_char(ch);
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

        KeyCode::Char('1') => app.set_tab(Tab::Ports),
        KeyCode::Char('2') => app.set_tab(Tab::Processes),
        KeyCode::Char('3') => app.set_tab(Tab::Docker),
        KeyCode::Char('4') => app.set_tab(Tab::Volumes),

        KeyCode::Tab => {
            let next_tab = match app.tab {
                Tab::Ports => Tab::Processes,
                Tab::Processes => Tab::Docker,
                Tab::Docker => Tab::Volumes,
                Tab::Volumes => Tab::Ports,
            };
            app.set_tab(next_tab);
        }

        KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
        KeyCode::PageUp => app.move_selection(-20),
        KeyCode::PageDown => app.move_selection(20),

        KeyCode::Home => {
            app.selected_row = 0;
            app.table_state.select(Some(0));
        }

        KeyCode::End => {
            let last = app.active_list_len().saturating_sub(1);
            app.selected_row = last;
            app.table_state.select(Some(last));
        }

        KeyCode::Char('x') | KeyCode::Char('X') if app.tab == Tab::Processes => {
            app.request_kill_selected_process();
        }

        KeyCode::Char('s') if app.tab == Tab::Docker => app.stop_selected_container(),
        KeyCode::Char('S') if app.tab == Tab::Docker => app.restart_selected_container(),
        KeyCode::Char('d') | KeyCode::Char('D') if app.tab == Tab::Docker => {
            app.request_remove_selected_container();
        }
        KeyCode::Char('d') | KeyCode::Char('D') if app.tab == Tab::Volumes => {
            app.request_remove_selected_volumes();
        }

        KeyCode::Enter | KeyCode::Char('g') if app.tab == Tab::Ports => {
            app.jump_from_selected_port();
        }
        KeyCode::Enter | KeyCode::Char('g') if app.tab == Tab::Volumes => {
            app.jump_from_selected_volume();
        }

        KeyCode::Char(' ') => app.toggle_mark_current(),
        KeyCode::Char('a') => app.mark_all(),
        KeyCode::Char('A') => app.unmark_all(),

        _ => {}
    }
}
