use ratatui::widgets::TableState;

use crate::config::Config;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Ports,
    DevProcesses,
    RegularProcesses,
    Docker,
    Volumes,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Navigation {
    pub tab: Tab,
    pub selected_row: usize,
    pub list_offset: usize,
    pub table_state: TableState,
}

impl Tab {
    pub fn title(self) -> &'static str {
        match self {
            Tab::Ports => "Ports",
            Tab::DevProcesses => "Dev Processes",
            Tab::RegularProcesses => "All Processes",
            Tab::Docker => "Docker",
            Tab::Volumes => "Volumes",
        }
    }
}

pub fn visible_tabs(config: &Config) -> Vec<Tab> {
    let mut tabs = vec![Tab::Ports, Tab::DevProcesses];
    if config.show_all_processes {
        tabs.push(Tab::RegularProcesses);
    }
    tabs.push(Tab::Docker);
    tabs.push(Tab::Volumes);
    tabs
}

pub fn tab_at_digit(config: &Config, digit: u8) -> Option<Tab> {
    let index = digit.checked_sub(1)? as usize;
    visible_tabs(config).into_iter().nth(index)
}

pub fn next_tab(config: &Config, current: Tab) -> Tab {
    let tabs = visible_tabs(config);
    let Some(pos) = tabs.iter().position(|&t| t == current) else {
        // current hidden (e.g. config flipped) → first visible
        return tabs[0];
    };
    tabs[(pos + 1) % tabs.len()]
}

#[allow(dead_code)]
pub fn digit_for_tab(config: &Config, tab: Tab) -> Option<usize> {
    visible_tabs(config)
        .into_iter()
        .position(|t| t == tab)
        .map(|i| i + 1)
}

impl Navigation {
    pub fn new() -> Self {
        Self {
            tab: Tab::Ports,
            selected_row: 0,
            list_offset: 0,
            table_state: TableState::default(),
        }
    }

    pub fn reset_position(&mut self) {
        self.selected_row = 0;
        self.list_offset = 0;
        self.table_state.select(Some(0));
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.reset_position();
    }

    pub fn move_by(&mut self, delta: isize, len: usize) {
        if len == 0 {
            return;
        }
        let max = len - 1;
        let next = (self.selected_row as isize + delta).clamp(0, max as isize) as usize;
        self.select_row(next);
    }

    pub fn select_row(&mut self, index: usize) {
        self.selected_row = index;
        self.list_offset = 0;
        self.table_state.select(Some(index));
    }

    pub fn clamp_to_len(&mut self, len: usize) {
        if len == 0 {
            self.selected_row = 0;
            self.list_offset = 0;
            self.table_state.select(None);
            return;
        }
        if self.selected_row >= len {
            self.selected_row = len - 1;
        }
        self.table_state.select(Some(self.selected_row))
    }

    pub fn ensure_visible(&mut self, viewport_rows: usize) {
        if viewport_rows == 0 {
            return;
        }
        if self.selected_row < self.list_offset {
            self.list_offset = self.selected_row;
        } else if self.selected_row >= self.list_offset + viewport_rows {
            self.list_offset = self.selected_row - viewport_rows + 1;
        }
    }
}

impl Default for Navigation {
    fn default() -> Self {
        Self::new()
    }
}
