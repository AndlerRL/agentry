#[allow(dead_code)]
/// Cursor position in the editor.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

impl Cursor {
    pub fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        }
    }

    pub fn move_right(&mut self, buffer: &crate::editor::Buffer) {
        let max_col = buffer.line_len(self.row).saturating_sub(0);
        if self.col < max_col {
            self.col += 1;
        }
    }

    pub fn move_down(&mut self) {
        self.row += 1;
    }

    #[allow(dead_code)]
    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
        }
    }

    pub fn move_down_within(&mut self, buffer: &crate::editor::Buffer) {
        if self.row < buffer.line_count() - 1 {
            self.row += 1;
            self.clamp(buffer);
        }
    }

    pub fn move_up_within(&mut self, buffer: &crate::editor::Buffer) {
        if self.row > 0 {
            self.row -= 1;
            self.clamp(buffer);
        }
    }

    pub fn move_end_of_line(&mut self, buffer: &crate::editor::Buffer) {
        self.col = buffer.line_len(self.row).saturating_sub(1).max(1);
    }

    pub fn move_last_line(&mut self, buffer: &crate::editor::Buffer) {
        self.row = buffer.line_count().saturating_sub(1);
        self.clamp(buffer);
    }

    pub fn move_word_forward(&mut self, buffer: &crate::editor::Buffer) {
        let line = buffer.get_line(self.row);
        let rest = &line[self.col.min(line.len())..];
        let mut chars = rest.char_indices().peekable();
        // Skip remaining chars of current word
        for (_, c) in chars.by_ref() {
            if c.is_whitespace() {
                break;
            }
        }
        // Skip whitespace, then set cursor to start of next word
        loop {
            match chars.peek() {
                Some(&(_idx, c)) if c.is_whitespace() => {
                    chars.next();
                }
                Some(&(idx, _)) => {
                    self.col += idx;
                    return;
                }
                None => break,
            }
        }
        // If we reached end of line, go to next line
        if self.row < buffer.line_count() - 1 {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn move_word_backward(&mut self, buffer: &crate::editor::Buffer) {
        if self.col == 0 {
            if self.row > 0 {
                self.row -= 1;
                self.col = buffer.line_len(self.row).saturating_sub(1);
            }
            return;
        }
        let line = buffer.get_line(self.row);
        let before = &line[..self.col.min(line.len())];
        // Find start of previous word
        let trimmed = before.trim_end_matches(|c: char| c.is_whitespace());
        if let Some(pos) = trimmed.rfind(|c: char| c.is_whitespace()) {
            self.col = pos + 1;
        } else {
            self.col = 0;
        }
    }

    pub fn move_word_end(&mut self, buffer: &crate::editor::Buffer) {
        let line = buffer.get_line(self.row);
        let rest = &line[self.col.min(line.len())..];
        let mut found = false;
        let mut chars = rest.char_indices().peekable();
        // Skip whitespace first
        while let Some(&(_, c)) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        // Find end of word
        for (i, c) in chars {
            if c.is_whitespace() {
                self.col = i.saturating_sub(1);
                found = true;
                break;
            }
        }
        if !found {
            self.col = line.len().saturating_sub(1);
        }
    }

    /// Clamp cursor to valid buffer position.
    pub fn clamp(&mut self, buffer: &crate::editor::Buffer) {
        if self.row >= buffer.line_count() {
            self.row = buffer.line_count().saturating_sub(1);
        }
        let max_col = buffer.line_len(self.row);
        if self.col > max_col {
            self.col = max_col;
        }
    }
}
