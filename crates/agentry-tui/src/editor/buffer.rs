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
