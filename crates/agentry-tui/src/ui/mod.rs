mod dashboard;
mod intro;

use ratatui::layout::Rect;

pub use dashboard::draw_dashboard;
pub use intro::draw_intro;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Agents,
    Prompts,
    Skills,
    Sync,
    OpenClaw,
    Audit,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Agents,
        Tab::Prompts,
        Tab::Skills,
        Tab::Sync,
        Tab::OpenClaw,
        Tab::Audit,
    ];

    pub fn _index(self) -> usize {
        match self {
            Tab::Agents => 0,
            Tab::Prompts => 1,
            Tab::Skills => 2,
            Tab::Sync => 3,
            Tab::OpenClaw => 4,
            Tab::Audit => 5,
        }
    }

    pub fn from_index(i: usize) -> Option<Tab> {
        Tab::ALL.get(i).copied()
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Agents => "Agents",
            Tab::Prompts => "Prompts",
            Tab::Skills => "Skills",
            Tab::Sync => "Sync",
            Tab::OpenClaw => "OpenClaw",
            Tab::Audit => "Audit",
        }
    }
}

fn _centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
