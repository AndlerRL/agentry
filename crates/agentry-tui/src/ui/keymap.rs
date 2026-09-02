use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::app::App;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiAction {
    Quit,
    Help,
    NextTab,
    PrevTab,
    JumpTab(usize),
    ListNext,
    ListPrev,
    Enter,
    New,
    Delete,
    Sync,
    Edit,
    Insert,
    Update,
    Remove,
    RunAudit,
    CycleAuditFilter,
    Github,
    CreateWorkspace,
    AddAgent,
    MethodPrev,
    MethodNext,
    ListVersions,
    Workflow,
}

pub struct KeyBinding {
    pub key: String,
    pub label: String,
    pub action: TuiAction,
    pub when: Option<fn(&App) -> bool>,
}

fn binding(key: &str, label: &str, action: TuiAction) -> KeyBinding {
    KeyBinding {
        key: key.to_string(),
        label: label.to_string(),
        action,
        when: None,
    }
}

fn scoped(when: fn(&App) -> bool, key: &str, label: &str, action: TuiAction) -> KeyBinding {
    KeyBinding {
        key: key.to_string(),
        label: label.to_string(),
        action,
        when: Some(when),
    }
}

fn global_bindings() -> Vec<KeyBinding> {
    vec![
        binding("q", "Quit", TuiAction::Quit),
        binding("?", "Help", TuiAction::Help),
        binding("Tab", "Next tab", TuiAction::NextTab),
        binding("BackTab", "Prev tab", TuiAction::PrevTab),
        binding("j", "Next item", TuiAction::ListNext),
        binding("k", "Prev item", TuiAction::ListPrev),
        binding("Down", "Next item", TuiAction::ListNext),
        binding("Up", "Prev item", TuiAction::ListPrev),
        binding("1", "Agents", TuiAction::JumpTab(0)),
        binding("2", "Prompts", TuiAction::JumpTab(1)),
        binding("3", "Skills", TuiAction::JumpTab(2)),
        binding("4", "Sync", TuiAction::JumpTab(3)),
        binding("5", "OpenClaw", TuiAction::JumpTab(4)),
        binding("6", "Audit", TuiAction::JumpTab(5)),
    ]
}

fn agents_bindings() -> Vec<KeyBinding> {
    let when = |app: &App| app.tab_index == 0;
    vec![
        scoped(when, "Enter", "Install", TuiAction::Enter),
        scoped(when, "u", "Update", TuiAction::Update),
        scoped(when, "r", "Remove", TuiAction::Remove),
        scoped(when, "v", "Versions", TuiAction::ListVersions),
        scoped(when, "Left", "Prev method", TuiAction::MethodPrev),
        scoped(when, "Right", "Next method", TuiAction::MethodNext),
    ]
}

fn prompts_bindings() -> Vec<KeyBinding> {
    let when = |app: &App| app.tab_index == 1;
    vec![
        scoped(when, "Enter", "Open", TuiAction::Enter),
        scoped(when, "e", "Edit", TuiAction::Edit),
        scoped(when, "n", "New", TuiAction::New),
        scoped(when, "d", "Delete", TuiAction::Delete),
    ]
}

fn skills_bindings() -> Vec<KeyBinding> {
    let when = |app: &App| app.tab_index == 2;
    vec![
        scoped(when, "Enter", "Install", TuiAction::Enter),
        scoped(when, "i", "Install", TuiAction::Insert),
        scoped(when, "u", "Update", TuiAction::Update),
        scoped(when, "r", "Remove", TuiAction::Remove),
        scoped(when, "g", "GitHub", TuiAction::Github),
    ]
}

fn sync_bindings() -> Vec<KeyBinding> {
    let when = |app: &App| app.tab_index == 3;
    vec![
        scoped(when, "s", "Run sync", TuiAction::Sync),
        scoped(when, "w", "Workflow", TuiAction::Workflow),
    ]
}

fn openclaw_bindings() -> Vec<KeyBinding> {
    let when = |app: &App| app.tab_index == 4;
    vec![
        scoped(when, "Enter", "Edit doc", TuiAction::Enter),
        scoped(when, "n", "New workspace", TuiAction::New),
        scoped(when, "c", "Create workspace", TuiAction::CreateWorkspace),
        scoped(when, "a", "Add agent", TuiAction::AddAgent),
    ]
}

fn audit_bindings() -> Vec<KeyBinding> {
    let when = |app: &App| app.tab_index == 5;
    vec![
        scoped(when, "r", "Run audit", TuiAction::RunAudit),
        scoped(when, "f", "Filter", TuiAction::CycleAuditFilter),
        scoped(when, "Enter", "Open finding", TuiAction::Enter),
    ]
}

pub fn bindings_for_tab(tab_index: usize, _app: &App) -> Vec<KeyBinding> {
    let mut bindings = global_bindings();
    match tab_index {
        0 => bindings.extend(agents_bindings()),
        1 => bindings.extend(prompts_bindings()),
        2 => bindings.extend(skills_bindings()),
        3 => bindings.extend(sync_bindings()),
        4 => bindings.extend(openclaw_bindings()),
        5 => bindings.extend(audit_bindings()),
        _ => {}
    }
    bindings
}

pub fn resolve(tab_index: usize, app: &App, key: &str) -> Option<TuiAction> {
    bindings_for_tab(tab_index, app)
        .into_iter()
        .find(|b| b.key == key && b.when.is_none_or(|f| f(app)))
        .map(|b| b.action)
}

pub fn bar_lines(tab_index: usize, app: &App, width: usize) -> Vec<Line<'static>> {
    const NAV_KEYS: [&str; 6] = ["j", "k", "Up", "Down", "Tab", "BackTab"];
    let owned: Vec<KeyBinding> = bindings_for_tab(tab_index, app)
        .into_iter()
        .filter(|b| b.when.is_none_or(|f| f(app)))
        .collect();
    let mut ordered: Vec<&KeyBinding> = Vec::with_capacity(owned.len());
    let mut rest: Vec<&KeyBinding> = Vec::new();
    for b in &owned {
        if NAV_KEYS.contains(&b.key.as_str()) {
            ordered.push(b);
        } else {
            rest.push(b);
        }
    }
    ordered.extend(rest);

    fn item_width(b: &KeyBinding) -> usize {
        b.key.len() + 1 + b.label.len() + 3
    }

    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for b in &ordered {
        let w = item_width(b);
        if !current.is_empty() && used + w > width {
            lines.push(std::mem::take(&mut current));
            used = 0;
        }
        if used + w > width && w > width {
            break;
        }
        used += w;
        current.push(Span::styled(
            b.key.clone(),
            Style::default().fg(Color::Yellow),
        ));
        current.push(Span::styled(" ", Style::default()));
        current.push(Span::styled(
            b.label.clone(),
            Style::default().fg(Color::DarkGray),
        ));
        current.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines.into_iter().take(2).map(Line::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::*;

    const GLOBAL_KEYS: [&str; 14] = [
        "q", "?", "Tab", "BackTab", "j", "k", "Up", "Down", "1", "2", "3", "4", "5", "6",
    ];

    fn scoped_pairs(bindings: &[KeyBinding]) -> Vec<(&str, TuiAction)> {
        bindings
            .iter()
            .filter(|b| b.when.is_some())
            .map(|b| (b.key.as_str(), b.action.clone()))
            .collect()
    }

    #[test]
    fn bindings_for_tab_non_empty_for_all_tabs() {
        let app = App::new();
        let expected_totals = [20, 18, 19, 16, 18, 17];
        for tab in 0..6 {
            let bindings = bindings_for_tab(tab, &app);
            assert!(!bindings.is_empty(), "tab {tab} has no bindings");
            assert_eq!(bindings.len(), expected_totals[tab], "tab {tab} count");
        }
    }

    #[test]
    fn tab_scoped_bindings_have_when_predicates() {
        let app = App::new();
        for tab in 0..6 {
            let bindings = bindings_for_tab(tab, &app);
            let scoped_count = bindings.iter().filter(|b| b.when.is_some()).count();
            assert!(scoped_count > 0, "tab {tab} has no scoped bindings");
            for binding in &bindings {
                if !GLOBAL_KEYS.contains(&binding.key.as_str()) {
                    assert!(
                        binding.when.is_some(),
                        "tab {tab} binding '{}' lacks a when predicate",
                        binding.key
                    );
                }
            }
        }
    }

    #[test]
    fn global_bindings_have_when_none() {
        let app = App::new();
        for tab in 0..6 {
            for binding in bindings_for_tab(tab, &app) {
                if GLOBAL_KEYS.contains(&binding.key.as_str()) {
                    assert!(
                        binding.when.is_none(),
                        "binding '{}' should be global on tab {tab}",
                        binding.key
                    );
                }
            }
        }
    }

    #[test]
    fn global_keys_match_handle_key() {
        let app = App::new();
        let bindings = bindings_for_tab(0, &app);
        let global: Vec<(&str, TuiAction)> = bindings
            .iter()
            .filter(|b| b.when.is_none())
            .map(|b| (b.key.as_str(), b.action.clone()))
            .collect();
        let expected: Vec<(&str, TuiAction)> = vec![
            ("q", TuiAction::Quit),
            ("?", TuiAction::Help),
            ("Tab", TuiAction::NextTab),
            ("BackTab", TuiAction::PrevTab),
            ("j", TuiAction::ListNext),
            ("k", TuiAction::ListPrev),
            ("Down", TuiAction::ListNext),
            ("Up", TuiAction::ListPrev),
            ("1", TuiAction::JumpTab(0)),
            ("2", TuiAction::JumpTab(1)),
            ("3", TuiAction::JumpTab(2)),
            ("4", TuiAction::JumpTab(3)),
            ("5", TuiAction::JumpTab(4)),
            ("6", TuiAction::JumpTab(5)),
        ];
        assert_eq!(global, expected);
    }

    #[test]
    fn tab_scoped_actions_match_handle_key() {
        let app = App::new();
        let cases: Vec<(usize, Vec<(&str, TuiAction)>)> = vec![
            (
                0,
                vec![
                    ("Enter", TuiAction::Enter),
                    ("u", TuiAction::Update),
                    ("r", TuiAction::Remove),
                    ("v", TuiAction::ListVersions),
                    ("Left", TuiAction::MethodPrev),
                    ("Right", TuiAction::MethodNext),
                ],
            ),
            (
                1,
                vec![
                    ("Enter", TuiAction::Enter),
                    ("e", TuiAction::Edit),
                    ("n", TuiAction::New),
                    ("d", TuiAction::Delete),
                ],
            ),
            (
                2,
                vec![
                    ("Enter", TuiAction::Enter),
                    ("i", TuiAction::Insert),
                    ("u", TuiAction::Update),
                    ("r", TuiAction::Remove),
                    ("g", TuiAction::Github),
                ],
            ),
            (3, vec![("s", TuiAction::Sync), ("w", TuiAction::Workflow)]),
            (
                4,
                vec![
                    ("Enter", TuiAction::Enter),
                    ("n", TuiAction::New),
                    ("c", TuiAction::CreateWorkspace),
                    ("a", TuiAction::AddAgent),
                ],
            ),
            (
                5,
                vec![
                    ("r", TuiAction::RunAudit),
                    ("f", TuiAction::CycleAuditFilter),
                    ("Enter", TuiAction::Enter),
                ],
            ),
        ];
        for (tab, expected) in cases {
            assert_eq!(
                scoped_pairs(&bindings_for_tab(tab, &app)),
                expected,
                "tab {tab}"
            );
        }
    }

    #[test]
    fn resolve_maps_quit_on_tab_zero() {
        let app = App::new();
        assert_eq!(resolve(0, &app, "q"), Some(TuiAction::Quit));
    }

    #[test]
    fn resolve_dual_maps_r_by_tab() {
        let mut app = App::new();
        app.tab_index = 0;
        assert_eq!(resolve(0, &app, "r"), Some(TuiAction::Remove));
        app.tab_index = 5;
        assert_eq!(resolve(5, &app, "r"), Some(TuiAction::RunAudit));
    }

    #[test]
    fn resolve_returns_none_for_unknown_key() {
        let app = App::new();
        assert_eq!(resolve(0, &app, "zzz"), None);
    }

    #[test]
    fn resolve_skips_failed_when_predicates() {
        let mut app = App::new();
        app.tab_index = 0;
        assert_eq!(resolve(5, &app, "r"), None);
        assert_eq!(resolve(5, &app, "q"), Some(TuiAction::Quit));
    }

    fn bar_text(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect()
    }

    #[test]
    fn bar_lines_returns_one_or_two_lines_per_tab() {
        let app = App::new();
        for tab in 0..6 {
            let lines = bar_lines(tab, &app, 200);
            assert!(!lines.is_empty(), "tab {tab}");
            assert!(lines.len() <= 2, "tab {tab} produced {} lines", lines.len());
        }
    }

    #[test]
    fn bar_lines_truncates_at_narrow_width() {
        let app = App::new();
        let wide = bar_lines(0, &app, 200);
        let narrow = bar_lines(0, &app, 40);
        let sep_count = |lines: &[Line<'static>]| bar_text(lines).matches(" · ").count();
        assert!(sep_count(&narrow) < sep_count(&wide));
    }

    #[test]
    fn bar_lines_orders_nav_keys_first() {
        let app = App::new();
        let text = bar_text(&bar_lines(0, &app, 200));
        let j = text.find("j Next item").expect("j");
        let k = text.find("k Prev item").expect("k");
        let enter = text.find("Enter Install").expect("Enter");
        assert!(j < k);
        assert!(k < enter);
    }

    #[test]
    fn bar_lines_zero_width_does_not_panic() {
        let app = App::new();
        for tab in 0..6 {
            let _ = bar_lines(tab, &app, 0);
        }
    }
}
