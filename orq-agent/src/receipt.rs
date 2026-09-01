use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecStatus {
    Succeeded,
    Failed,
    TimedOut,
    Blocked,
    SpawnFailed,
    InvalidRequest,
}

#[derive(Debug, Serialize)]
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
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn tail_sanitized(input: &[u8], max_bytes: usize) -> String {
    let start = input.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&input[start..])
        .replace('\0', "")
        .replace('\r', "")
}

#[cfg(test)]
mod tests {
    use super::tail_sanitized;

    #[test]
    fn tail_is_bounded() {
        let text = vec![b'a'; 10];
        assert_eq!(tail_sanitized(&text, 4), "aaaa");
    }
}
