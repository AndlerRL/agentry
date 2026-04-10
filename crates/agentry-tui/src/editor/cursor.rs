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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Buffer;

    fn make_buffer() -> Buffer {
        Buffer::from_content("hello world\nfoo bar\nlast line")
    }

    #[test]
    fn move_left_decrements_col() {
        let mut cursor = Cursor { row: 0, col: 3 };
        cursor.move_left();
        assert_eq!(cursor.col, 2);
    }

    #[test]
    fn move_left_clamps_at_zero() {
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_left();
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn move_left_clamps_at_zero_repeatedly() {
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_left();
        cursor.move_left();
        cursor.move_left();
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn move_right_advances_within_line() {
        let buf = Buffer::from_content("hello");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_right(&buf);
        assert_eq!(cursor.col, 1);
    }

    #[test]
    fn move_right_stops_at_end_of_line() {
        let buf = Buffer::from_content("hi");
        // line_len(0) = 2, saturating_sub(0) = 2, so max_col = 2
        // move_right advances while col < max_col
        let mut cursor = Cursor { row: 0, col: 1 };
        cursor.move_right(&buf);
        assert_eq!(cursor.col, 2); // advances to col 2 (one past last char)
                                   // Now at max_col, further moves should not advance
        cursor.move_right(&buf);
        assert_eq!(cursor.col, 2);
    }

    #[test]
    fn move_right_on_empty_line_stays_at_zero() {
        let buf = Buffer::new(); // single empty line, len = 0
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_right(&buf);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn move_down_increments_row() {
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_down();
        assert_eq!(cursor.row, 1);
    }

    #[test]
    fn move_up_decrements_row() {
        let mut cursor = Cursor { row: 2, col: 0 };
        cursor.move_up();
        assert_eq!(cursor.row, 1);
    }

    #[test]
    fn move_up_clamps_at_zero() {
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_up();
        assert_eq!(cursor.row, 0);
    }

    #[test]
    fn move_down_within_advances_row() {
        let buf = make_buffer();
        let mut cursor = Cursor { row: 0, col: 5 };
        cursor.move_down_within(&buf);
        assert_eq!(cursor.row, 1);
    }

    #[test]
    fn move_down_within_clamps_col_to_shorter_line() {
        // Row 0: "hello world" (len 11), Row 1: "foo bar" (len 7)
        let buf = Buffer::from_content("hello world\nfoo bar");
        let mut cursor = Cursor { row: 0, col: 10 };
        cursor.move_down_within(&buf);
        assert_eq!(cursor.row, 1);
        // After clamp, col should be capped at line_len(1)=7
        assert_eq!(cursor.col, 7);
    }

    #[test]
    fn move_down_within_does_not_go_past_last_line() {
        let buf = Buffer::from_content("only one line");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_down_within(&buf);
        assert_eq!(cursor.row, 0);
    }

    #[test]
    fn move_up_within_decrements_row() {
        let buf = make_buffer();
        let mut cursor = Cursor { row: 2, col: 0 };
        cursor.move_up_within(&buf);
        assert_eq!(cursor.row, 1);
    }

    #[test]
    fn move_up_within_does_not_go_above_first_line() {
        let buf = make_buffer();
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_up_within(&buf);
        assert_eq!(cursor.row, 0);
    }

    #[test]
    fn move_up_within_clamps_col_to_shorter_line() {
        // Row 1: "foo bar" (len 7), Row 0: "hi" (len 2)
        let buf = Buffer::from_content("hi\nfoo bar");
        let mut cursor = Cursor { row: 1, col: 6 };
        cursor.move_up_within(&buf);
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 2); // clamped to line_len(0)
    }

    #[test]
    fn move_end_of_line_on_non_empty_line() {
        let buf = Buffer::from_content("hello");
        // line_len(0) = 5, saturating_sub(1).max(1) = 4
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_end_of_line(&buf);
        assert_eq!(cursor.col, 4);
    }

    #[test]
    fn move_end_of_line_on_empty_line() {
        // Empty line has len 0, saturating_sub(1).max(1) = 1
        let buf = Buffer::new();
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_end_of_line(&buf);
        assert_eq!(cursor.col, 1);
    }

    #[test]
    fn move_last_line_goes_to_last_row() {
        let buf = Buffer::from_content("a\nb\nc");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_last_line(&buf);
        assert_eq!(cursor.row, 2);
    }

    #[test]
    fn move_last_line_clamps_col() {
        // Last line is "ab" (len 2), if col > 2 it should be clamped
        let buf = Buffer::from_content("hello world\nab");
        let mut cursor = Cursor { row: 0, col: 10 };
        cursor.move_last_line(&buf);
        assert_eq!(cursor.row, 1);
        assert!(cursor.col <= 2);
    }

    #[test]
    fn move_word_forward_skips_to_next_word() {
        let buf = Buffer::from_content("hello world");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_word_forward(&buf);
        assert_eq!(cursor.col, 6); // start of "world"
    }

    #[test]
    fn move_word_forward_at_end_of_line_goes_to_next_line() {
        let buf = Buffer::from_content("hello\nworld");
        let mut cursor = Cursor { row: 0, col: 0 };
        // Move forward: "hello" is one word, goes to end of line then to next line
        cursor.move_word_forward(&buf);
        assert_eq!(cursor.row, 1);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn move_word_forward_on_last_word_stays() {
        let buf = Buffer::from_content("hello");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_word_forward(&buf);
        // "hello" is the only word, and we're on the last line, so cursor stays
        assert_eq!(cursor.row, 0);
        // col may advance past the word to end of line
    }

    #[test]
    fn move_word_backward_from_middle_of_word() {
        let buf = Buffer::from_content("hello world");
        let mut cursor = Cursor { row: 0, col: 8 }; // inside "world"
        cursor.move_word_backward(&buf);
        assert_eq!(cursor.col, 6); // start of "world"
    }

    #[test]
    fn move_word_backward_from_start_of_word() {
        let buf = Buffer::from_content("hello world");
        let mut cursor = Cursor { row: 0, col: 6 }; // start of "world"
        cursor.move_word_backward(&buf);
        assert_eq!(cursor.col, 0); // start of "hello"
    }

    #[test]
    fn move_word_backward_at_start_of_line_goes_to_previous_line() {
        let buf = Buffer::from_content("hello\nworld");
        let mut cursor = Cursor { row: 1, col: 0 };
        cursor.move_word_backward(&buf);
        assert_eq!(cursor.row, 0);
        // col goes to end of previous line (len-1 via saturating_sub)
        assert_eq!(cursor.col, 4);
    }

    #[test]
    fn move_word_backward_at_col_zero_first_line_stays() {
        let buf = Buffer::from_content("hello");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_word_backward(&buf);
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn move_word_end_skips_to_end_of_next_word() {
        let buf = Buffer::from_content("hello world");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_word_end(&buf);
        // Should land at the last char of "hello" which is index 4
        assert_eq!(cursor.col, 4);
    }

    #[test]
    fn move_word_end_from_whitespace() {
        let buf = Buffer::from_content("  hello");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_word_end(&buf);
        // Skips whitespace, then finds end of "hello" at index 6
        assert_eq!(cursor.col, 6);
    }

    #[test]
    fn move_word_end_single_word() {
        let buf = Buffer::from_content("abc");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_word_end(&buf);
        // Only one word, should land at end
        assert_eq!(cursor.col, 2);
    }

    #[test]
    fn clamp_row_out_of_bounds() {
        let buf = Buffer::from_content("hello\nworld");
        let mut cursor = Cursor { row: 10, col: 0 };
        cursor.clamp(&buf);
        assert_eq!(cursor.row, 1);
    }

    #[test]
    fn clamp_col_out_of_bounds() {
        let buf = Buffer::from_content("hi");
        let mut cursor = Cursor { row: 0, col: 50 };
        cursor.clamp(&buf);
        assert_eq!(cursor.col, 2);
    }

    #[test]
    fn clamp_both_out_of_bounds() {
        let buf = Buffer::from_content("ab\ncd");
        let mut cursor = Cursor { row: 99, col: 99 };
        cursor.clamp(&buf);
        assert_eq!(cursor.row, 1);
        assert_eq!(cursor.col, 2);
    }

    #[test]
    fn clamp_within_bounds_does_nothing() {
        let buf = Buffer::from_content("hello\nworld");
        let mut cursor = Cursor { row: 0, col: 3 };
        cursor.clamp(&buf);
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 3);
    }

    #[test]
    fn clamp_col_to_empty_line() {
        // Empty line has len 0, col should be clamped to 0
        let buf = Buffer::from_content("hello\n\nworld");
        let mut cursor = Cursor { row: 1, col: 5 };
        cursor.clamp(&buf);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn default_cursor_is_at_origin() {
        let cursor = Cursor::default();
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn multiple_moves_combo() {
        let buf = Buffer::from_content("hello world\nfoo bar");
        let mut cursor = Cursor { row: 0, col: 0 };
        cursor.move_right(&buf);
        cursor.move_right(&buf);
        cursor.move_down_within(&buf);
        assert_eq!(cursor.row, 1);
        assert!(cursor.col <= 2); // clamped to line 1 length
    }
}
