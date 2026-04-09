#[allow(dead_code)]
/// Editor mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Visual,
    Command,
}