#[allow(dead_code)]
/// Text buffer for the editor. Stores lines as a Vec<String>.
#[derive(Debug, Clone)]
pub struct Buffer {
    lines: Vec<String>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
        }
    }

    pub fn from_content(content: &str) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };
        // Ensure at least one line
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Self { lines }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn get_line(&self, row: usize) -> &str {
        self.lines.get(row).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn line_len(&self, row: usize) -> usize {
        self.lines.get(row).map(|s| s.len()).unwrap_or(0)
    }

    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(|s| s.as_str())
    }

    pub fn insert_char_at(&mut self, row: usize, col: usize, c: char) {
        if let Some(line) = self.lines.get_mut(row) {
            if col <= line.len() {
                line.insert(col, c);
            }
        }
    }

    pub fn delete_char_at(&mut self, row: usize, col: usize) {
        if let Some(line) = self.lines.get_mut(row) {
            if col < line.len() {
                line.remove(col);
            }
        }
    }

    pub fn insert_newline_at(&mut self, row: usize, col: usize) {
        if row < self.lines.len() {
            let line = &mut self.lines[row];
            let right: String = line.drain(col..).collect();
            self.lines.insert(row + 1, right);
        }
    }

    pub fn insert_newline_below(&mut self, row: usize) {
        self.lines.insert(row + 1, String::new());
    }

    pub fn insert_newline_above(&mut self, row: usize) {
        self.lines.insert(row, String::new());
    }

    pub fn delete_line(&mut self, row: usize) {
        if self.lines.len() > 1 {
            self.lines.remove(row);
        } else {
            self.lines[0] = String::new();
        }
    }

    pub fn join_lines(&mut self, row: usize) {
        if row + 1 < self.lines.len() {
            let next = self.lines.remove(row + 1);
            if let Some(line) = self.lines.get_mut(row) {
                line.push_str(&next);
            }
        }
    }

    pub fn content(&self) -> String {
        self.lines.join("\n")
    }
}

impl std::fmt::Display for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_buffer_with_one_empty_line() {
        let buf = Buffer::new();
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.get_line(0), "");
    }

    #[test]
    fn from_content_empty_string_has_one_empty_line() {
        let buf = Buffer::from_content("");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.get_line(0), "");
    }

    #[test]
    fn from_content_single_line_no_trailing_newline() {
        let buf = Buffer::from_content("hello");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.get_line(0), "hello");
    }

    #[test]
    fn from_content_two_lines() {
        let buf = Buffer::from_content("hello\nworld");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.get_line(0), "hello");
        assert_eq!(buf.get_line(1), "world");
    }

    #[test]
    fn from_content_trailing_newline() {
        // "hello\n" -> lines() gives ["hello"], trailing newline is dropped by str::lines()
        let buf = Buffer::from_content("hello\n");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.get_line(0), "hello");
    }

    #[test]
    fn from_content_multiple_lines() {
        let buf = Buffer::from_content("a\nb\nc");
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.get_line(0), "a");
        assert_eq!(buf.get_line(1), "b");
        assert_eq!(buf.get_line(2), "c");
    }

    #[test]
    fn line_count_works() {
        let buf = Buffer::from_content("one\ntwo\nthree");
        assert_eq!(buf.line_count(), 3);
    }

    #[test]
    fn get_line_out_of_bounds_returns_empty() {
        let buf = Buffer::from_content("hello");
        assert_eq!(buf.get_line(5), "");
    }

    #[test]
    fn line_len_works() {
        let buf = Buffer::from_content("hi\nworld");
        assert_eq!(buf.line_len(0), 2);
        assert_eq!(buf.line_len(1), 5);
        assert_eq!(buf.line_len(999), 0); // out of bounds
    }

    #[test]
    fn lines_iterator_yields_all_lines() {
        let buf = Buffer::from_content("aa\nbb\ncc");
        let collected: Vec<&str> = buf.lines().collect();
        assert_eq!(collected, vec!["aa", "bb", "cc"]);
    }

    #[test]
    fn insert_char_at_beginning() {
        let mut buf = Buffer::from_content("ello");
        buf.insert_char_at(0, 0, 'h');
        assert_eq!(buf.get_line(0), "hello");
    }

    #[test]
    fn insert_char_at_middle() {
        let mut buf = Buffer::from_content("hllo");
        buf.insert_char_at(0, 1, 'e');
        assert_eq!(buf.get_line(0), "hello");
    }

    #[test]
    fn insert_char_at_end() {
        let mut buf = Buffer::from_content("hell");
        buf.insert_char_at(0, 4, 'o');
        assert_eq!(buf.get_line(0), "hello");
    }

    #[test]
    fn insert_char_at_out_of_bounds_row_ignored() {
        let mut buf = Buffer::from_content("hello");
        buf.insert_char_at(5, 0, 'x');
        assert_eq!(buf.get_line(0), "hello");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn insert_char_at_out_of_bounds_col_ignored() {
        let mut buf = Buffer::from_content("hi");
        buf.insert_char_at(0, 10, 'x'); // col beyond line len, should be ignored
        assert_eq!(buf.get_line(0), "hi");
    }

    #[test]
    fn delete_char_at_removes_char() {
        let mut buf = Buffer::from_content("hello");
        buf.delete_char_at(0, 1); // delete 'e'
        assert_eq!(buf.get_line(0), "hllo");
    }

    #[test]
    fn delete_char_at_beginning() {
        let mut buf = Buffer::from_content("abc");
        buf.delete_char_at(0, 0);
        assert_eq!(buf.get_line(0), "bc");
    }

    #[test]
    fn delete_char_at_end() {
        let mut buf = Buffer::from_content("abc");
        buf.delete_char_at(0, 2);
        assert_eq!(buf.get_line(0), "ab");
    }

    #[test]
    fn delete_char_at_out_of_bounds_ignored() {
        let mut buf = Buffer::from_content("abc");
        buf.delete_char_at(0, 3); // col == len, out of bounds for removal
        assert_eq!(buf.get_line(0), "abc");
    }

    #[test]
    fn delete_char_at_out_of_bounds_row_ignored() {
        let mut buf = Buffer::from_content("abc");
        buf.delete_char_at(5, 0);
        assert_eq!(buf.get_line(0), "abc");
    }

    #[test]
    fn insert_newline_at_splits_line() {
        let mut buf = Buffer::from_content("helloworld");
        buf.insert_newline_at(0, 5);
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.get_line(0), "hello");
        assert_eq!(buf.get_line(1), "world");
    }

    #[test]
    fn insert_newline_at_beginning_of_line() {
        let mut buf = Buffer::from_content("hello");
        buf.insert_newline_at(0, 0);
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.get_line(0), "");
        assert_eq!(buf.get_line(1), "hello");
    }

    #[test]
    fn insert_newline_at_end_of_line() {
        let mut buf = Buffer::from_content("hello");
        buf.insert_newline_at(0, 5);
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.get_line(0), "hello");
        assert_eq!(buf.get_line(1), "");
    }

    #[test]
    fn insert_newline_at_out_of_bounds_row_ignored() {
        let mut buf = Buffer::from_content("hello");
        buf.insert_newline_at(5, 0);
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn insert_newline_below_adds_empty_line() {
        let mut buf = Buffer::from_content("line1\nline2");
        buf.insert_newline_below(0);
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.get_line(0), "line1");
        assert_eq!(buf.get_line(1), "");
        assert_eq!(buf.get_line(2), "line2");
    }

    #[test]
    fn insert_newline_above_adds_empty_line() {
        let mut buf = Buffer::from_content("line1\nline2");
        buf.insert_newline_above(1);
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.get_line(0), "line1");
        assert_eq!(buf.get_line(1), "");
        assert_eq!(buf.get_line(2), "line2");
    }

    #[test]
    fn insert_newline_above_at_row_zero() {
        let mut buf = Buffer::from_content("hello");
        buf.insert_newline_above(0);
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.get_line(0), "");
        assert_eq!(buf.get_line(1), "hello");
    }

    #[test]
    fn delete_line_on_multi_line_buffer_removes_line() {
        let mut buf = Buffer::from_content("line1\nline2\nline3");
        buf.delete_line(1);
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.get_line(0), "line1");
        assert_eq!(buf.get_line(1), "line3");
    }

    #[test]
    fn delete_line_on_single_line_buffer_clears_it() {
        let mut buf = Buffer::from_content("hello");
        buf.delete_line(0);
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.get_line(0), "");
    }

    #[test]
    fn delete_line_first_line() {
        let mut buf = Buffer::from_content("first\nsecond");
        buf.delete_line(0);
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.get_line(0), "second");
    }

    #[test]
    fn delete_line_last_line() {
        let mut buf = Buffer::from_content("first\nsecond");
        buf.delete_line(1);
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.get_line(0), "first");
    }

    #[test]
    fn join_lines_merges_two_lines() {
        let mut buf = Buffer::from_content("hello\nworld");
        buf.join_lines(0);
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.get_line(0), "helloworld");
    }

    #[test]
    fn join_lines_last_row_does_nothing() {
        let mut buf = Buffer::from_content("hello\nworld");
        buf.join_lines(1); // no row after this
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.get_line(0), "hello");
        assert_eq!(buf.get_line(1), "world");
    }

    #[test]
    fn join_lines_out_of_bounds_does_nothing() {
        let mut buf = Buffer::from_content("hello\nworld");
        buf.join_lines(5);
        assert_eq!(buf.line_count(), 2);
    }

    #[test]
    fn content_reconstructs_original() {
        let original = "hello\nworld";
        let buf = Buffer::from_content(original);
        assert_eq!(buf.content(), original);
    }

    #[test]
    fn content_single_line() {
        let buf = Buffer::from_content("just one line");
        assert_eq!(buf.content(), "just one line");
    }

    #[test]
    fn content_empty_buffer() {
        let buf = Buffer::new();
        assert_eq!(buf.content(), "");
    }

    #[test]
    fn display_trait_formats_content() {
        let buf = Buffer::from_content("abc\ndef");
        assert_eq!(format!("{}", buf), "abc\ndef");
    }

    #[test]
    fn display_trait_empty_buffer() {
        let buf = Buffer::new();
        assert_eq!(format!("{}", buf), "");
    }

    #[test]
    fn multiple_operations_consistency() {
        let mut buf = Buffer::new();
        buf.insert_char_at(0, 0, 'a');
        buf.insert_char_at(0, 1, 'b');
        buf.insert_char_at(0, 2, 'c');
        assert_eq!(buf.get_line(0), "abc");
        buf.insert_newline_at(0, 1);
        assert_eq!(buf.get_line(0), "a");
        assert_eq!(buf.get_line(1), "bc");
        buf.join_lines(0);
        assert_eq!(buf.get_line(0), "abc");
        assert_eq!(buf.line_count(), 1);
    }
}
