use std::collections::HashSet;
use std::path::Path;

use agentry_audit::fix::{default_allowlist, validate_with_allowlist};
use agentry_audit::report::{AuditFinding, FindingCategory, FixAction, Severity};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RawFinding {
    #[serde(default)]
    pub check_id: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub remediation: Option<String>,
    #[serde(default)]
    pub auto_fixable: Option<bool>,
    #[serde(default)]
    pub fix: Option<FixAction>,
    #[serde(default)]
    pub suggested_fix: Option<FixAction>,
    #[serde(default)]
    pub evidence: Option<String>,
    #[serde(default)]
    pub skill_request: Option<String>,
}

pub fn extract_last_json_array(response: &str) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    let mut rest = response;
    while let Some(start) = rest.find('[') {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (index, ch) in rest[start..].char_indices() {
            match ch {
                '"' if !escaped => in_string = !in_string,
                '\\' if in_string && !escaped => escaped = true,
                '\\' => {}
                '[' if !in_string => depth += 1,
                ']' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        let end = start + index + 1;
                        candidates.push(rest[start..end].to_string());
                        rest = &rest[end..];
                        break;
                    }
                }
                _ => {}
            }
            if ch != '\\' {
                escaped = false;
            }
        }
        if depth != 0 {
            break;
        }
    }
    candidates.pop()
}

pub fn sanitize_finding(
    raw: RawFinding,
    home_dir: &Path,
    allowlist: &[std::path::PathBuf],
) -> Option<AuditFinding> {
    if let Some(skill_request) = raw.skill_request {
        if !skill_request.trim().is_empty() {
            return Some(AuditFinding {
                check_id: "auditor.skill_request".to_string(),
                severity: Severity::Suggestion,
                category: FindingCategory::Audited,
                agent_id: raw.agent_id,
                message: format!("auditor requests skill '{skill_request}'"),
                remediation: format!("agentry skills install {skill_request}"),
                auto_fixable: false,
                fix: None,
                suggested_fix: None,
                evidence: Some(skill_request),
            });
        }
    }
    let check_id = raw.check_id.unwrap_or_default().trim().to_string();
    if check_id.is_empty() {
        return None;
    }
    let check_id = if check_id.starts_with("auditor.") {
        check_id
    } else {
        format!("auditor.{check_id}")
    };
    let message = raw.message.unwrap_or_default();
    if message.trim().is_empty() {
        return None;
    }
    let suggested_fix = raw
        .suggested_fix
        .or(raw.fix)
        .filter(|fix| validate_with_allowlist(fix, home_dir, allowlist).is_ok());
    Some(AuditFinding {
        check_id,
        severity: Severity::Suggestion,
        category: FindingCategory::Audited,
        agent_id: raw.agent_id,
        message,
        remediation: raw.remediation.unwrap_or_default(),
        auto_fixable: false,
        fix: None,
        suggested_fix,
        evidence: raw.evidence,
    })
}

#[derive(Debug, Clone)]
pub enum ParseReport {
    Unparseable,
    Findings(Vec<AuditFinding>),
}

pub fn parse_findings(response: &str, home_dir: &Path, max_findings: usize) -> ParseReport {
    let Some(json) = extract_last_json_array(response) else {
        return ParseReport::Unparseable;
    };
    let raw: Vec<RawFinding> = match serde_json::from_str(&json) {
        Ok(raw) => raw,
        Err(_) => return ParseReport::Unparseable,
    };
    let allowlist = default_allowlist(home_dir);
    let mut seen: HashSet<String> = HashSet::new();
    let mut findings: Vec<AuditFinding> = Vec::new();
    for item in raw {
        if findings.len() >= max_findings {
            break;
        }
        let Some(finding) = sanitize_finding(item, home_dir, &allowlist) else {
            continue;
        };
        if !seen.insert(finding.check_id.clone()) {
            continue;
        }
        findings.push(finding);
    }
    ParseReport::Findings(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(prefix: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn extracts_last_fenced_array() {
        let response = "Here is the analysis:\n```json\n[{\"check_id\":\"a\"}]\n```\nAnd more text";
        let json = extract_last_json_array(response).unwrap();
        assert!(json.contains("\"check_id\":\"a\""));
    }

    #[test]
    fn extracts_last_bare_array() {
        let response = "prefix [{\"check_id\":\"a\"},{\"check_id\":\"b\"}] suffix";
        let json = extract_last_json_array(response).unwrap();
        assert!(json.contains("\"check_id\":\"b\""));
    }

    #[test]
    fn extracts_last_array_when_multiple_present() {
        let response = "[{\"check_id\":\"first\"}] trailing [{\"check_id\":\"second\"}]";
        let json = extract_last_json_array(response).unwrap();
        assert!(json.contains("\"check_id\":\"second\""));
        assert!(!json.contains("first"));
    }

    #[test]
    fn returns_none_without_array() {
        assert!(extract_last_json_array("no json here").is_none());
    }

    #[test]
    fn sanitize_forces_audited_suggestion_and_strips_fix() {
        let home = temp_home("agentry_test_parse_sanitize");
        let raw = RawFinding {
            check_id: Some("my_check".to_string()),
            severity: Some("critical".to_string()),
            category: Some("config".to_string()),
            agent_id: None,
            message: Some("problem".to_string()),
            remediation: Some("fix it".to_string()),
            auto_fixable: Some(true),
            fix: Some(FixAction::ShellCommand {
                description: "d".to_string(),
                command: "echo foo; echo injected".to_string(),
            }),
            suggested_fix: None,
            evidence: Some("e".to_string()),
            skill_request: None,
        };
        let allowlist = default_allowlist(&home);
        let finding = sanitize_finding(raw, &home, &allowlist).unwrap();
        assert_eq!(finding.check_id, "auditor.my_check");
        assert_eq!(finding.severity, Severity::Suggestion);
        assert_eq!(finding.category, FindingCategory::Audited);
        assert!(!finding.auto_fixable);
        assert!(finding.fix.is_none());
        assert!(finding.suggested_fix.is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn sanitize_keeps_gate_valid_suggested_fix() {
        let home = temp_home("agentry_test_parse_valid_fix");
        let allowlist = default_allowlist(&home);
        let raw = RawFinding {
            check_id: Some("write".to_string()),
            severity: None,
            category: None,
            agent_id: None,
            message: Some("m".to_string()),
            remediation: None,
            auto_fixable: None,
            fix: None,
            suggested_fix: Some(FixAction::FileWrite {
                path: home.join(".agents").join("prompts").join("X.md"),
                content: "body".to_string(),
            }),
            evidence: None,
            skill_request: None,
        };
        let finding = sanitize_finding(raw, &home, &allowlist).unwrap();
        assert!(finding.suggested_fix.is_some());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn sanitize_drops_out_of_bounds_suggested_fix() {
        let home = temp_home("agentry_test_parse_oob");
        let allowlist = default_allowlist(&home);
        let raw = RawFinding {
            check_id: Some("write".to_string()),
            severity: None,
            category: None,
            agent_id: None,
            message: Some("m".to_string()),
            remediation: Some("remediation survives".to_string()),
            auto_fixable: None,
            fix: None,
            suggested_fix: Some(FixAction::FileWrite {
                path: std::path::PathBuf::from("/etc/passwd"),
                content: "pwned".to_string(),
            }),
            evidence: None,
            skill_request: None,
        };
        let finding = sanitize_finding(raw, &home, &allowlist).unwrap();
        assert!(finding.suggested_fix.is_none());
        assert_eq!(finding.remediation, "remediation survives");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn parse_dedupes_and_caps() {
        let home = temp_home("agentry_test_parse_cap");
        let mut items = Vec::new();
        for i in 0..5 {
            items.push(format!("{{\"check_id\":\"dup\",\"message\":\"m{i}\"}}"));
        }
        for i in 0..5 {
            items.push(format!(
                "{{\"check_id\":\"unique{i}\",\"message\":\"m{i}\"}}"
            ));
        }
        let response = format!("[{}]", items.join(","));
        let ParseReport::Findings(findings) = parse_findings(&response, &home, 3) else {
            panic!("expected findings");
        };
        assert_eq!(findings.len(), 3);
        let ids: Vec<&str> = findings.iter().map(|f| f.check_id.as_str()).collect();
        assert_eq!(ids, ["auditor.dup", "auditor.unique0", "auditor.unique1"]);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn parse_handles_skill_request() {
        let home = temp_home("agentry_test_parse_skill");
        let response = r#"[{"skill_request":"context-engineering-collection"}]"#;
        let ParseReport::Findings(findings) = parse_findings(response, &home, 20) else {
            panic!("expected findings");
        };
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].check_id, "auditor.skill_request");
        assert_eq!(findings[0].severity, Severity::Suggestion);
        assert!(findings[0].remediation.contains("agentry skills install"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn parse_marks_garbage_unparseable() {
        let home = temp_home("agentry_test_parse_garbage");
        assert!(matches!(
            parse_findings("not json at all", &home, 20),
            ParseReport::Unparseable
        ));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn parse_distinguishes_valid_empty_array() {
        let home = temp_home("agentry_test_parse_empty");
        assert!(matches!(
            parse_findings("[]", &home, 20),
            ParseReport::Findings(ref findings) if findings.is_empty()
        ));
        assert!(matches!(
            parse_findings("here is the verdict: []", &home, 20),
            ParseReport::Findings(ref findings) if findings.is_empty()
        ));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn parse_distinguishes_unparseable_array() {
        let home = temp_home("agentry_test_parse_bad_array");
        assert!(matches!(
            parse_findings("[{\"check_id\":]", &home, 20),
            ParseReport::Unparseable
        ));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
