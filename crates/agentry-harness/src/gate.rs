use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::action::HarnessError;

pub struct GateTicket {
    pub action_id: String,
    pub consent_id: String,
    pub minted_at: chrono::DateTime<chrono::Utc>,
}

impl GateTicket {
    pub(crate) fn new(action_id: String, consent_id: String) -> Self {
        Self {
            action_id,
            consent_id,
            minted_at: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecord {
    pub consent_id: String,
    pub action_id: String,
    pub granted_at: chrono::DateTime<chrono::Utc>,
    pub decision: String,
}

pub fn consent_path(home_dir: &std::path::Path) -> PathBuf {
    home_dir
        .join(".agents")
        .join("agentry")
        .join("consent.jsonl")
}

pub fn record_consent(
    home_dir: &std::path::Path,
    action_id: &str,
    decision: &str,
) -> Result<String, HarnessError> {
    let record = ConsentRecord {
        consent_id: String::new(),
        action_id: action_id.to_string(),
        granted_at: chrono::Utc::now(),
        decision: decision.to_string(),
    };
    let serialized = serde_json::to_string(&record)
        .map_err(|err| HarnessError::ExecutionFailed(format!("consent serialize failed: {err}")))?;
    let consent_id = consent_id_for(&serialized);
    let record = ConsentRecord {
        consent_id: consent_id.clone(),
        ..record
    };
    let serialized = serde_json::to_string(&record)
        .map_err(|err| HarnessError::ExecutionFailed(format!("consent serialize failed: {err}")))?;
    let path = consent_path(home_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            HarnessError::ExecutionFailed(format!("failed to create {}: {err}", parent.display()))
        })?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| {
            HarnessError::ExecutionFailed(format!("failed to open {}: {err}", path.display()))
        })?;
    writeln!(file, "{serialized}").map_err(|err| {
        HarnessError::ExecutionFailed(format!("failed to write {}: {err}", path.display()))
    })?;
    Ok(consent_id)
}

fn consent_id_for(serialized: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(serialized.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    hex[..12].to_string()
}

pub fn load_consents(home_dir: &std::path::Path) -> Result<Vec<ConsentRecord>, HarnessError> {
    let path = consent_path(home_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(HarnessError::ExecutionFailed(format!(
                "failed to read {}: {err}",
                path.display()
            )))
        }
    };
    let mut records = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let record: ConsentRecord = serde_json::from_str(line).map_err(|err| {
            HarnessError::ExecutionFailed(format!(
                "malformed consent record in {}: {err}",
                path.display()
            ))
        })?;
        records.push(record);
    }
    Ok(records)
}

pub fn consent_id_matches(
    home_dir: &std::path::Path,
    consent_id: &str,
) -> Result<bool, HarnessError> {
    Ok(load_consents(home_dir)?
        .iter()
        .any(|record| record.consent_id == consent_id))
}

pub fn assert_ticket_for(
    ticket: &GateTicket,
    action_id: &str,
    home_dir: &std::path::Path,
) -> Result<(), HarnessError> {
    if ticket.action_id != action_id {
        return Err(HarnessError::TicketMismatch {
            ticket_id: ticket.action_id.clone(),
            action_id: action_id.to_string(),
        });
    }
    if !consent_id_matches(home_dir, &ticket.consent_id)? {
        return Err(HarnessError::ExecutionFailed(format!(
            "consent record {} not found for action '{}'",
            ticket.consent_id, action_id
        )));
    }
    Ok(())
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
    fn ticket_ctor_is_crate_internal_but_fields_readable() {
        let ticket = GateTicket::new("audit.run".to_string(), "abc123".to_string());
        assert_eq!(ticket.action_id, "audit.run");
        assert_eq!(ticket.consent_id, "abc123");
    }

    #[test]
    fn record_consent_appends_jsonl_and_returns_id() {
        let home = temp_home("agentry_test_gate_record");
        let id1 = record_consent(&home, "sync.execute", "granted").unwrap();
        let id2 = record_consent(&home, "fix.apply", "granted").unwrap();
        assert_ne!(id1, id2);
        let path = consent_path(&home);
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        let record: ConsentRecord = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(record.action_id, "sync.execute");
        assert_eq!(record.decision, "granted");
        assert_eq!(record.consent_id, id1);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn load_consents_returns_empty_when_missing() {
        let home = temp_home("agentry_test_gate_empty");
        let records = load_consents(&home).unwrap();
        assert!(records.is_empty());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn load_consents_reads_back_records() {
        let home = temp_home("agentry_test_gate_readback");
        record_consent(&home, "audit.run", "granted").unwrap();
        let records = load_consents(&home).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].action_id, "audit.run");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn load_consents_fails_closed_on_malformed_line() {
        let home = temp_home("agentry_test_gate_malformed");
        let path = consent_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json\n").unwrap();
        assert!(load_consents(&home).is_err());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn assert_ticket_for_rejects_mismatched_action() {
        let home = temp_home("agentry_test_gate_mismatch");
        let consent_id = record_consent(&home, "audit.run", "granted").unwrap();
        let ticket = GateTicket::new("fix.apply".to_string(), consent_id);
        let err = assert_ticket_for(&ticket, "audit.run", &home).unwrap_err();
        assert!(matches!(err, HarnessError::TicketMismatch { .. }));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn assert_ticket_for_rejects_missing_consent_record() {
        let home = temp_home("agentry_test_gate_missing_consent");
        let ticket = GateTicket::new("audit.run".to_string(), "deadbeef0000".to_string());
        let err = assert_ticket_for(&ticket, "audit.run", &home).unwrap_err();
        assert!(err.to_string().contains("consent record"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn assert_ticket_for_accepts_valid_ticket() {
        let home = temp_home("agentry_test_gate_valid");
        let consent_id = record_consent(&home, "sync.execute", "granted").unwrap();
        let ticket = GateTicket::new("sync.execute".to_string(), consent_id);
        assert!(assert_ticket_for(&ticket, "sync.execute", &home).is_ok());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
