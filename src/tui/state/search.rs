#[derive(Debug, Default, Clone)]
pub struct SearchState {
    pub query: String,
    pub match_index: usize,
}

impl SearchState {
    pub fn clear(&mut self) {
        self.query.clear();
        self.match_index = 0;
    }

    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
    }
    pub fn pop_char(&mut self) {
        self.query.pop();
    }
}
