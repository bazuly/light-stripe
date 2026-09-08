use ratatui::widgets::TableState;

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
