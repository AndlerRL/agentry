mod buffer;
mod cursor;
mod mode;

pub use buffer::Buffer;
pub use cursor::Cursor;
pub use mode::EditorMode;

use std::collections::VecDeque;

#[allow(dead_code)]
/// Vim-like text editor state.
pub struct Editor {
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub mode: EditorMode,
    pub filename: Option<String>,
    pub modified: bool,
    pub command_buf: String,
    pub message: Option<String>,
    pub search_query: Option<String>,
    pub search_direction: SearchDirection,
    /// Undo history (stores previous buffer states)
    undo_stack: VecDeque<String>,
    /// Max undo depth
    max_undo: usize,
    /// Scroll offset (first visible line in viewport)
    pub scroll_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SearchDirection {
    Forward,
    Backward,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor::default(),
            mode: EditorMode::Normal,
            filename: None,
            modified: false,
            command_buf: String::new(),
            message: None,
            search_query: None,
            search_direction: SearchDirection::Forward,
            undo_stack: VecDeque::new(),
            max_undo: 100,
            scroll_offset: 0,
        }
    }

    pub fn with_content(content: &str) -> Self {
        let mut editor = Self::new();
        editor.buffer = Buffer::from_content(content);
        editor
    }

    /// Save current state to undo stack before a modification.
    fn push_undo(&mut self) {
        if self.undo_stack.len() >= self.max_undo {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(self.buffer.content());
    }

    /// Handle a key input in the current mode.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        match self.mode {
            EditorMode::Normal => self.handle_normal_key(key),
            EditorMode::Insert => self.handle_insert_key(key),
            EditorMode::Visual => self.handle_visual_key(key),
            EditorMode::Command => self.handle_command_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        let code = key.code;
        let modifiers = key.modifiers;

        match (code, modifiers) {
            // Mode switches
            (KeyCode::Char('i'), _) => {
                self.mode = EditorMode::Insert;
            }
            (KeyCode::Char('a'), _) => {
                self.cursor.move_right(&self.buffer);
                self.mode = EditorMode::Insert;
            }
            (KeyCode::Char('o'), _) => {
                self.push_undo();
                self.buffer.insert_newline_below(self.cursor.row);
                self.cursor.move_down();
                self.cursor.col = 0;
                self.mode = EditorMode::Insert;
            }
            (KeyCode::Char('O'), _) => {
                self.push_undo();
                self.buffer.insert_newline_above(self.cursor.row);
                self.cursor.col = 0;
                self.mode = EditorMode::Insert;
            }
            (KeyCode::Char('v'), _) => {
                self.mode = EditorMode::Visual;
            }
            (KeyCode::Char(':'), _) => {
                self.mode = EditorMode::Command;
                self.command_buf.clear();
            }
            (KeyCode::Char('/'), _) => {
                self.mode = EditorMode::Command;
                self.command_buf.clear();
                self.command_buf.push('/');
            }

            // Motions
            (KeyCode::Char('h'), _) | (KeyCode::Left, _) => self.cursor.move_left(),
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => {
                self.cursor.move_down_within(&self.buffer)
            }
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => self.cursor.move_up_within(&self.buffer),
            (KeyCode::Char('l'), _) | (KeyCode::Right, _) => self.cursor.move_right(&self.buffer),
            (KeyCode::Char('w'), _) => self.cursor.move_word_forward(&self.buffer),
            (KeyCode::Char('b'), _) => self.cursor.move_word_backward(&self.buffer),
            (KeyCode::Char('e'), _) => self.cursor.move_word_end(&self.buffer),
            (KeyCode::Char('0'), _) | (KeyCode::Home, _) => self.cursor.col = 0,
            (KeyCode::Char('$'), _) => self.cursor.move_end_of_line(&self.buffer),
            (KeyCode::Char('G'), _) => self.cursor.move_last_line(&self.buffer),
            (KeyCode::Char('g'), _) => {
                self.cursor.row = 0;
                self.cursor.col = 0;
            } // gg - simplified

            // Editing
            (KeyCode::Char('x'), _) => {
                self.push_undo();
                self.buffer.delete_char_at(self.cursor.row, self.cursor.col);
                self.cursor.clamp(&self.buffer);
            }
            (KeyCode::Char('d'), _) => {
                // dd - delete line (simplified, just 'd' for now; full dd requires key chords)
                self.push_undo();
                self.buffer.delete_line(self.cursor.row);
                self.cursor.clamp(&self.buffer);
                self.modified = true;
            }
            (KeyCode::Char('y'), _) => {
                // yy - yank line (just store in clipboard concept)
                self.message = Some("Yanked line".into());
            }
            (KeyCode::Char('p'), _) => {
                // p - paste after (placeholder)
            }
            (KeyCode::Char('u'), _) => {
                if let Some(prev) = self.undo_stack.pop_back() {
                    self.buffer = Buffer::from_content(&prev);
                    self.cursor.clamp(&self.buffer);
                    self.message = Some("Undo".into());
                }
            }

            // Search
            (KeyCode::Char('n'), _) => {
                let query = self.search_query.clone();
                if let Some(ref q) = query {
                    self.search_forward(q);
                }
            }

            _ => {}
        }
    }

    fn handle_insert_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
            }
            KeyCode::Enter => {
                self.push_undo();
                self.buffer
                    .insert_newline_at(self.cursor.row, self.cursor.col);
                self.cursor.move_down();
                self.cursor.col = 0;
                self.modified = true;
            }
            KeyCode::Backspace => {
                if self.cursor.col > 0 {
                    self.push_undo();
                    self.buffer
                        .delete_char_at(self.cursor.row, self.cursor.col - 1);
                    self.cursor.move_left();
                    self.modified = true;
                } else if self.cursor.row > 0 {
                    self.push_undo();
                    let len_prev = self.buffer.line_len(self.cursor.row - 1);
                    self.buffer.join_lines(self.cursor.row - 1);
                    self.cursor.move_up_within(&self.buffer);
                    self.cursor.col = len_prev;
                    self.modified = true;
                }
            }
            KeyCode::Char(c) => {
                self.push_undo();
                self.buffer
                    .insert_char_at(self.cursor.row, self.cursor.col, c);
                self.cursor.move_right(&self.buffer);
                self.modified = true;
            }
            KeyCode::Tab => {
                self.push_undo();
                for _ in 0..4 {
                    self.buffer
                        .insert_char_at(self.cursor.row, self.cursor.col, ' ');
                    self.cursor.col += 1;
                }
                self.modified = true;
            }
            _ => {}
        }
    }

    fn handle_visual_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        if key.code == KeyCode::Esc {
            self.mode = EditorMode::Normal;
        }
    }

    fn handle_command_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
            }
            KeyCode::Enter => {
                self.execute_command(&self.command_buf.clone());
                self.mode = EditorMode::Normal;
            }
            KeyCode::Backspace => {
                self.command_buf.pop();
                if self.command_buf.is_empty() {
                    self.mode = EditorMode::Normal;
                }
            }
            KeyCode::Char(c) => {
                self.command_buf.push(c);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd: &str) {
        let cmd = cmd.trim_start_matches(':').trim_start_matches('/');
        match cmd {
            "w" => self.save(),
            "q" => {
                self.message = Some("Use Esc to exit editor".into());
            }
            "wq" => {
                self.save();
                self.message = Some("Saved.".into());
            }
            s if s.starts_with("s/") => {
                // Simple :%s/old/new/ substitute
                self.push_undo();
                if let Some(substituted) = self.substitute(s) {
                    self.message = Some(format!("Substituted: {}", substituted));
                    self.modified = true;
                }
            }
            s if !s.is_empty() && !s.starts_with('/') => {
                // Search
                self.search_query = Some(s.to_string());
                self.search_forward(s);
            }
            _ => {}
        }
    }

    fn save(&mut self) {
        // In a real implementation, this would write to disk
        self.modified = false;
        self.message = Some(format!(
            "Saved{}",
            self.filename
                .as_ref()
                .map(|f| format!(" {}", f))
                .unwrap_or_default()
        ));
    }

    fn substitute(&mut self, cmd: &str) -> Option<String> {
        // :%s/old/new/ or :s/old/new/
        let cmd = cmd.trim_start_matches('%');
        let parts: Vec<&str> = cmd.split('/').collect();
        if parts.len() >= 3 {
            let find = parts.get(1)?;
            let replace = parts.get(2)?;
            let content = self.buffer.content();
            let new_content = content.replace(find, replace);
            let count = content.matches(find).count();
            self.buffer = Buffer::from_content(&new_content);
            Some(format!("{} replacement(s)", count))
        } else {
            None
        }
    }

    fn search_forward(&mut self, query: &str) {
        let start_row = self.cursor.row + 1;
        for (i, _line) in self.buffer.lines().enumerate() {
            let row = (start_row + i) % self.buffer.line_count();
            if let Some(pos) = self.buffer.get_line(row).find(query) {
                self.cursor.row = row;
                self.cursor.col = pos;
                self.message = Some(format!("Found at line {}", row + 1));
                return;
            }
        }
        self.message = Some("Pattern not found".into());
    }

    /// Get the display content for the editor (line numbers + content).
    pub fn render_lines(&self, viewport_start: usize, viewport_height: usize) -> Vec<String> {
        let line_count = self.buffer.line_count();
        let line_num_width = line_count.to_string().len().max(3);

        let mut lines = Vec::new();
        for i in viewport_start..(viewport_start + viewport_height) {
            if i < line_count {
                let line_num = format!("{:>width$} ", i + 1, width = line_num_width);
                let content = self.buffer.get_line(i);
                lines.push(format!("{}{}", line_num, content));
            } else {
                lines.push("~".repeat(line_num_width + 1));
            }
        }
        lines
    }

    /// Status line content.
    pub fn status_line(&self) -> String {
        let mode = match self.mode {
            EditorMode::Normal => "NORMAL",
            EditorMode::Insert => "INSERT",
            EditorMode::Visual => "VISUAL",
            EditorMode::Command => "COMMAND",
        };
        let modified = if self.modified { " [+]" } else { "" };
        let filename = self.filename.as_deref().unwrap_or("[No Name]");
        let line = self.cursor.row + 1;
        let col = self.cursor.col + 1;
        format!(
            "{} │ {}:{}{} │ L{},C{}",
            mode, filename, line, modified, line, col
        )
    }

    /// Update scroll offset so the cursor remains visible in the viewport.
    /// Uses a 5-line context margin at top/bottom of viewport.
    #[allow(dead_code)]
    pub fn ensure_cursor_visible(&mut self, viewport_height: usize) {
        let margin = 5.min(viewport_height / 4);
        if self.cursor.row < self.scroll_offset + margin {
            self.scroll_offset = self.cursor.row.saturating_sub(margin);
        } else if self.cursor.row >= self.scroll_offset + viewport_height - margin {
            self.scroll_offset = self.cursor.row + margin + 1 - viewport_height;
        }
    }
}
