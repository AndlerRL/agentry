mod dashboard;
mod intro;
pub mod keymap;

pub use dashboard::draw_dashboard;
pub use intro::draw_intro;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Agents,
    Prompts,
    Skills,
    Sync,
    Audit,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Agents,
        Tab::Prompts,
        Tab::Skills,
        Tab::Sync,
        Tab::Audit,
    ];

    pub fn from_index(i: usize) -> Option<Tab> {
        Tab::ALL.get(i).copied()
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Agents => "Agents",
            Tab::Prompts => "Prompts",
            Tab::Skills => "Skills",
            Tab::Sync => "Sync",
            Tab::Audit => "Audit",
        }
    }
}
