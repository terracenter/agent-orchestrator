use color_eyre::eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecStatus {
    Succeeded,
    Failed,
    TimedOut,
    Blocked,
    SpawnFailed,
    InvalidRequest,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExecReceipt {
    pub schema_version: u8,
    pub correlation_id: String,
    pub agent: String,
    pub model: String,
    pub command: Vec<String>,
    pub status: ExecStatus,
    pub policy_reason: String,
    pub started_at_unix: u64,
    pub duration_ms: u128,
    pub timeout_seconds: u64,
    pub exit_code: Option<i32>,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub secrets_read: bool,
    #[serde(default)]
    pub cleanup_attempted: bool,
    #[serde(default)]
    pub cleanup_succeeded: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegateStatus {
    Planned,
    CommandGenerated,
    Executed,
    Validated,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegateVerdict {
    Util,
    NonUtil,
    Indeterminado,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DelegateReceipt {
    pub schema_version: u8,
    pub correlation_id: String,
    pub agent: String,
    pub model: String,
    pub command: Vec<String>,
    pub status: DelegateStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub verdict: DelegateVerdict,
    pub evidence: String,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub started_at_unix: u64,
    pub duration_ms: u128,
    pub timeout_seconds: u64,
    pub exit_code: Option<i32>,
    pub secrets_read: bool,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn now_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

pub fn receipt_sha256(receipt: &ExecReceipt) -> Result<String> {
    let receipt_json = serde_json::to_vec(receipt).wrap_err("serializing receipt for sha256")?;
    Ok(hex_sha256(&receipt_json))
}

#[allow(dead_code)]
pub fn delegate_receipt_sha256(receipt: &DelegateReceipt) -> Result<String> {
    let receipt_json =
        serde_json::to_vec(receipt).wrap_err("serializing delegate receipt for sha256")?;
    Ok(hex_sha256(&receipt_json))
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn tail_sanitized(input: &[u8], max_bytes: usize) -> String {
    let start = input.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&input[start..]).replace(['\0', '\r'], "")
}

#[cfg(test)]
mod tests {
    use super::{
        delegate_receipt_sha256, receipt_sha256, tail_sanitized, DelegateReceipt, DelegateStatus,
        DelegateVerdict, ExecReceipt, ExecStatus,
    };

    #[test]
    fn tail_is_bounded() {
        let text = vec![b'a'; 10];
        assert_eq!(tail_sanitized(&text, 4), "aaaa");
    }

    #[test]
    fn receipt_hash_is_stable_sha256_hex() {
        let receipt = ExecReceipt {
            schema_version: 1,
            correlation_id: "receipt-hash-test".to_string(),
            agent: "test-agent".to_string(),
            model: "test-model".to_string(),
            command: vec!["true".to_string()],
            status: ExecStatus::Succeeded,
            policy_reason: "allowed".to_string(),
            started_at_unix: 1,
            duration_ms: 2,
            timeout_seconds: 3,
            exit_code: Some(0),
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
        };

        let hash = receipt_sha256(&receipt).expect("hash receipt");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn delegate_receipt_serialization_and_hash() {
        let receipt = DelegateReceipt {
            schema_version: 1,
            correlation_id: "del-test-123".to_string(),
            agent: "agy".to_string(),
            model: "gemini-3.7-flash-high".to_string(),
            command: vec!["rtk".to_string(), "agy".to_string()],
            status: DelegateStatus::Validated,
            reason: None,
            verdict: DelegateVerdict::Util,
            evidence: "a1b2c3d4e5f6".to_string(),
            stdout_tail: "done".to_string(),
            stderr_tail: String::new(),
            started_at_unix: 1000,
            duration_ms: 500,
            timeout_seconds: 30,
            exit_code: Some(0),
            secrets_read: false,
        };

        let serialized = serde_json::to_string(&receipt).expect("serialize");
        assert!(serialized.contains("\"status\":\"validated\""));
        assert!(serialized.contains("\"verdict\":\"util\""));
        assert!(serialized.contains("\"evidence\":\"a1b2c3d4e5f6\""));

        let hash = delegate_receipt_sha256(&receipt).expect("hash delegate receipt");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|ch| ch.is_ascii_hexdigit()));
    }
}
