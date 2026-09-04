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

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn receipt_sha256(receipt: &ExecReceipt) -> Result<String> {
    let receipt_json = serde_json::to_vec(receipt).wrap_err("serializing receipt for sha256")?;
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
    use super::{receipt_sha256, tail_sanitized, ExecReceipt, ExecStatus};

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
}
