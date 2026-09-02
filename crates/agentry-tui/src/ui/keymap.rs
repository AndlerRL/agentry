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
    SyncExecuteSelected,
    SyncExecuteAll,
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
    CancelVersion,
    Harness(HarnessInvocation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessInvocation {
    pub action_id: String,
    pub input_json: String,
}

impl HarnessInvocation {
    pub fn new(action_id: &str, input: &agentry_harness::ActionInput) -> Self {
        let input_json = serde_json::to_string(input).unwrap_or_default();
        Self {
            action_id: action_id.to_string(),
            input_json,
        }
    }
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

pub fn global_bindings() -> Vec<KeyBinding> {
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
        binding("5", "Audit", TuiAction::JumpTab(4)),
        scoped(
            |app: &App| app.version_list.is_some(),
            "Esc",
            "Cancel version",
            TuiAction::CancelVersion,
        ),
    ]
}

fn agents_bindings() -> Vec<KeyBinding> {
    let when = |app: &App| app.tab_index == 0;
    let openclaw = |app: &App| app.tab_index == 0 && app.selected_agent_is_openclaw();
    let not_openclaw = |app: &App| app.tab_index == 0 && !app.selected_agent_is_openclaw();
    vec![
        scoped(openclaw, "Enter", "Edit doc", TuiAction::Enter),
        scoped(not_openclaw, "Enter", "Install", TuiAction::Enter),
        scoped(when, "u", "Update", TuiAction::Update),
        scoped(when, "r", "Remove", TuiAction::Remove),
        scoped(when, "v", "Versions", TuiAction::ListVersions),
        scoped(when, "Left", "Prev method", TuiAction::MethodPrev),
        scoped(when, "Right", "Next method", TuiAction::MethodNext),
        scoped(
            openclaw,
            "c",
            "Create workspace",
            TuiAction::CreateWorkspace,
        ),
        scoped(openclaw, "a", "Add agent", TuiAction::AddAgent),
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
    let loaded = |app: &App| app.tab_index == 3 && app.sync_loaded;
    vec![
        scoped(
            loaded,
            "s",
            "Execute selected",
            TuiAction::SyncExecuteSelected,
        ),
        scoped(loaded, "S", "Execute all", TuiAction::SyncExecuteAll),
        scoped(
            loaded,
            "Enter",
            "Execute selected (alias of s)",
            TuiAction::SyncExecuteSelected,
        ),
    ]
}

fn audit_bindings() -> Vec<KeyBinding> {
    let when = |app: &App| app.tab_index == 4;
    let fixable = |app: &App| {
        app.tab_index == 4
            && app.selected_finding().is_some_and(|f| {
                (f.auto_fixable && f.fix.is_some())
                    || (f.category == agentry_audit::report::FindingCategory::Audited
                        && f.suggested_fix.is_some())
            })
    };
    let any_fixable = |app: &App| {
        app.tab_index == 4
            && app
                .audit_report
                .as_ref()
                .is_some_and(|report| !agentry_audit::fix::fixable_findings(report).is_empty())
    };
    let auditor_ready = |app: &App| app.tab_index == 4 && app.audit_loaded;
    vec![
        scoped(when, "r", "Run audit", TuiAction::RunAudit),
        scoped(when, "f", "Filter", TuiAction::CycleAuditFilter),
        scoped(when, "Enter", "Open finding", TuiAction::Enter),
        scoped(
            auditor_ready,
            "l",
            "Auditor review",
            TuiAction::Harness(HarnessInvocation::new(
                "auditor.review",
                &agentry_harness::ActionInput::AuditorReview {
                    focus_check_id: None,
                },
            )),
        ),
        scoped(
            auditor_ready,
            "L",
            "Auditor review",
            TuiAction::Harness(HarnessInvocation::new(
                "auditor.review",
                &agentry_harness::ActionInput::AuditorReview {
                    focus_check_id: None,
                },
            )),
        ),
        scoped(
            fixable,
            "a",
            "Apply fix",
            TuiAction::Harness(HarnessInvocation::new(
                "fix.apply",
                &agentry_harness::ActionInput::FixApply {
                    check_id: String::new(),
                },
            )),
        ),
        scoped(
            any_fixable,
            "A",
            "Apply all fixes",
            TuiAction::Harness(HarnessInvocation::new(
                "fix.apply_all",
                &agentry_harness::ActionInput::FixApplyAll,
            )),
        ),
    ]
}

pub fn bindings_for_tab(tab_index: usize, _app: &App) -> Vec<KeyBinding> {
    let mut bindings = global_bindings();
    match tab_index {
        0 => bindings.extend(agents_bindings()),
        1 => bindings.extend(prompts_bindings()),
        2 => bindings.extend(skills_bindings()),
        3 => bindings.extend(sync_bindings()),
        4 => bindings.extend(audit_bindings()),
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

pub fn confirm_bar_lines() -> Vec<Line<'static>> {
    bar_from_entries(&[("y", "Confirm"), ("n", "Cancel"), ("Esc", "Cancel")])
}

pub fn input_bar_lines() -> Vec<Line<'static>> {
    bar_from_entries(&[("Enter", "Commit"), ("Esc", "Cancel")])
}

fn bar_from_entries(entries: &[(&str, &str)]) -> Vec<Line<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (key, label) in entries {
        spans.push(Span::styled(
            key.to_string(),
            Style::default().fg(Color::Yellow),
        ));
        spans.push(Span::styled(" ", Style::default()));
        spans.push(Span::styled(
            label.to_string(),
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
    }
    vec![Line::from(spans)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLOBAL_KEYS: [&str; 13] = [
        "q", "?", "Tab", "BackTab", "j", "k", "Up", "Down", "1", "2", "3", "4", "5",
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
        let expected_totals = [23, 18, 19, 17, 21];
        for (tab, expected) in expected_totals.iter().enumerate() {
            let bindings = bindings_for_tab(tab, &app);
            assert!(!bindings.is_empty(), "tab {tab} has no bindings");
            assert_eq!(bindings.len(), *expected, "tab {tab} count");
        }
    }

    #[test]
    fn tab_scoped_bindings_have_when_predicates() {
        let app = App::new();
        for tab in 0..5 {
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
        for tab in 0..5 {
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
                    ("Esc", TuiAction::CancelVersion),
                    ("Enter", TuiAction::Enter),
                    ("Enter", TuiAction::Enter),
                    ("u", TuiAction::Update),
                    ("r", TuiAction::Remove),
                    ("v", TuiAction::ListVersions),
                    ("Left", TuiAction::MethodPrev),
                    ("Right", TuiAction::MethodNext),
                    ("c", TuiAction::CreateWorkspace),
                    ("a", TuiAction::AddAgent),
                ],
            ),
            (
                1,
                vec![
                    ("Esc", TuiAction::CancelVersion),
                    ("Enter", TuiAction::Enter),
                    ("e", TuiAction::Edit),
                    ("n", TuiAction::New),
                    ("d", TuiAction::Delete),
                ],
            ),
            (
                2,
                vec![
                    ("Esc", TuiAction::CancelVersion),
                    ("Enter", TuiAction::Enter),
                    ("i", TuiAction::Insert),
                    ("u", TuiAction::Update),
                    ("r", TuiAction::Remove),
                    ("g", TuiAction::Github),
                ],
            ),
            (
                3,
                vec![
                    ("Esc", TuiAction::CancelVersion),
                    ("s", TuiAction::SyncExecuteSelected),
                    ("S", TuiAction::SyncExecuteAll),
                    ("Enter", TuiAction::SyncExecuteSelected),
                ],
            ),
            (
                4,
                vec![
                    ("Esc", TuiAction::CancelVersion),
                    ("r", TuiAction::RunAudit),
                    ("f", TuiAction::CycleAuditFilter),
                    ("Enter", TuiAction::Enter),
                    (
                        "l",
                        TuiAction::Harness(HarnessInvocation::new(
                            "auditor.review",
                            &agentry_harness::ActionInput::AuditorReview {
                                focus_check_id: None,
                            },
                        )),
                    ),
                    (
                        "L",
                        TuiAction::Harness(HarnessInvocation::new(
                            "auditor.review",
                            &agentry_harness::ActionInput::AuditorReview {
                                focus_check_id: None,
                            },
                        )),
                    ),
                    (
                        "a",
                        TuiAction::Harness(HarnessInvocation::new(
                            "fix.apply",
                            &agentry_harness::ActionInput::FixApply {
                                check_id: String::new(),
                            },
                        )),
                    ),
                    (
                        "A",
                        TuiAction::Harness(HarnessInvocation::new(
                            "fix.apply_all",
                            &agentry_harness::ActionInput::FixApplyAll,
                        )),
                    ),
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
        app.tab_index = 4;
        assert_eq!(resolve(4, &app, "r"), Some(TuiAction::RunAudit));
    }

    #[test]
    fn resolve_returns_none_for_unknown_key() {
        let app = App::new();
        assert_eq!(resolve(0, &app, "zzz"), None);
    }

    #[test]
    fn resolve_esc_maps_cancel_version_only_when_version_list_loaded() {
        let app = App::new();
        for tab in 0..5 {
            assert_eq!(resolve(tab, &app, "Esc"), None, "tab {tab}");
        }
        let mut picking = App::new();
        picking.version_list = Some(vec!["1.0.0".to_string()]);
        for tab in 0..5 {
            assert_eq!(
                resolve(tab, &picking, "Esc"),
                Some(TuiAction::CancelVersion),
                "tab {tab}"
            );
        }
    }

    #[test]
    fn resolve_skips_failed_when_predicates() {
        let mut app = App::new();
        app.tab_index = 0;
        assert_eq!(resolve(4, &app, "r"), None);
        assert_eq!(resolve(4, &app, "q"), Some(TuiAction::Quit));
    }

    #[test]
    fn resolve_workflow_key_returns_none_on_all_tabs() {
        let app = App::new();
        for tab in 0..5 {
            assert_eq!(resolve(tab, &app, "w"), None, "tab {tab}");
        }
    }

    #[test]
    fn resolve_auditor_keys_gated_on_audit_loaded() {
        let app = App::new();
        for tab in 0..5 {
            assert_eq!(resolve(tab, &app, "l"), None, "tab {tab}");
            assert_eq!(resolve(tab, &app, "L"), None, "tab {tab}");
        }
        let mut loaded = App::new();
        loaded.tab_index = 4;
        loaded.audit_loaded = true;
        assert!(matches!(
            resolve(4, &loaded, "l"),
            Some(TuiAction::Harness(_))
        ));
        assert!(matches!(
            resolve(4, &loaded, "L"),
            Some(TuiAction::Harness(_))
        ));
    }

    #[test]
    fn resolve_apply_a_widened_to_audited_suggested_fix() {
        let mut app = App::new();
        app.tab_index = 4;
        app.audit_loaded = true;
        let json = r#"{"generated_at":"2026-01-01T00:00:00Z","machine_id":"m","agents":[],"global_findings":[{"check_id":"auditor.write","severity":"suggestion","category":"audited","agent_id":null,"message":"m","remediation":"r","auto_fixable":false,"fix":null,"suggested_fix":{"kind":"file_write","path":"/home/user/.agents/x.md","content":"body"},"evidence":null}],"summary":{"total_findings":1,"by_severity":{},"by_category":{},"auto_fixable_count":0,"healthy_agents":0,"degraded_agents":0},"schema_version":2}"#;
        app.audit_report = Some(serde_json::from_str(json).unwrap());
        app.list_selected = 1;
        assert!(matches!(resolve(4, &app, "a"), Some(TuiAction::Harness(_))));
    }

    #[test]
    fn resolve_method_prev_gated_to_agents_tab() {
        let app = App::new();
        assert_eq!(resolve(0, &app, "Left"), Some(TuiAction::MethodPrev));
        for tab in 1..5 {
            assert_eq!(resolve(tab, &app, "Left"), None, "tab {tab}");
        }
    }

    #[test]
    fn resolve_insert_inert_outside_skills_tab() {
        let app = App::new();
        for tab in [0, 1, 3, 4] {
            assert_eq!(resolve(tab, &app, "i"), None, "tab {tab}");
        }
        let mut skills_app = App::new();
        skills_app.tab_index = 2;
        assert_eq!(resolve(2, &skills_app, "i"), Some(TuiAction::Insert));
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
        for tab in 0..5 {
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
        for tab in 0..5 {
            let _ = bar_lines(tab, &app, 0);
        }
    }

    #[test]
    fn bar_lines_spans_exactly_match_filtered_bindings_for_all_tabs() {
        for tab in 0..5 {
            let app = fixture_app();
            let lines = bar_lines(tab, &app, 200);
            assert!(lines.len() <= 2, "tab {tab}: too many lines");

            let bar_pairs: Vec<(String, String)> = lines
                .iter()
                .flat_map(|l| l.spans.chunks(4))
                .map(|c| {
                    (
                        c[0].content.to_string(),
                        c[1].content.to_string() + &c[2].content,
                    )
                })
                .collect();

            let bindings = bindings_for_tab(tab, &app);
            let mut nav: Vec<&KeyBinding> = Vec::new();
            let mut rest: Vec<&KeyBinding> = Vec::new();
            for b in bindings.iter().filter(|b| b.when.is_none_or(|f| f(&app))) {
                if matches!(
                    b.key.as_str(),
                    "j" | "k" | "Up" | "Down" | "Tab" | "BackTab"
                ) {
                    nav.push(b);
                } else {
                    rest.push(b);
                }
            }
            nav.extend(rest);

            let expected: Vec<(String, String)> = nav
                .into_iter()
                .map(|b| (b.key.clone(), format!(" {}", b.label)))
                .collect();

            assert_eq!(bar_pairs.len(), expected.len(), "tab {tab}: item count");
            assert_eq!(
                bar_pairs, expected,
                "tab {tab}: bar spans diverge from registry"
            );
        }
    }

    fn fixture_app() -> App {
        let mut app = App::new();
        app.sync_loaded = true;
        app
    }
}
