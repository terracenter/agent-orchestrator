use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceStatus {
    Ok,
    Violation,
    NotAvailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub status: ComplianceStatus,
    pub exit_code: i32,
    pub checks: ComplianceChecks,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComplianceChecks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtk_usage: Option<RtkUsageReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engram_summary: Option<EngramSummaryReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vg_sync: Option<VgSyncReport>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RtkViolation {
    pub file: String,
    pub line: usize,
    pub raw_command: String,
    pub binary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RtkUsageReport {
    pub status: ComplianceStatus,
    pub raw_invocations_count: usize,
    pub scanned_files_count: usize,
    pub violations: Vec<RtkViolation>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngramSummaryReport {
    pub status: ComplianceStatus,
    pub project: String,
    pub target_date: String,
    pub session_summaries_count: usize,
    pub engram_binary: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VgSyncReport {
    pub status: ComplianceStatus,
    pub vault_path: String,
    pub kuzu_path: String,
    pub vault_head_unix: Option<u64>,
    pub kuzu_sync_unix: Option<u64>,
    pub is_fresh: bool,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct ComplianceArgs {
    pub log: Option<String>,
    pub project: Option<String>,
    pub vault_path: Option<String>,
    pub kuzu_path: Option<String>,
    pub engram_bin: Option<String>,
    pub rtk_usage: bool,
    pub engram_summary: bool,
    pub vg_sync: bool,
    pub agent: Option<String>,
}

pub async fn run_compliance(args: ComplianceArgs) -> Result<ComplianceReport> {
    let run_all = !args.rtk_usage && !args.engram_summary && !args.vg_sync;

    let rtk_report = if run_all || args.rtk_usage {
        Some(check_rtk_usage(args.log.as_deref())?)
    } else {
        None
    };

    let engram_report = if run_all || args.engram_summary {
        Some(check_engram_summary(args.project.as_deref(), args.engram_bin.as_deref()).await?)
    } else {
        None
    };

    let vg_report = if run_all || args.vg_sync {
        Some(check_vg_sync(args.vault_path.as_deref(), args.kuzu_path.as_deref())?)
    } else {
        None
    };

    let mut has_violation = false;
    let mut violation_reasons = Vec::new();

    if let Some(ref r) = rtk_report {
        if r.status == ComplianceStatus::Violation {
            has_violation = true;
            violation_reasons.push(format!("rtk-usage: {} raw invocation(s)", r.raw_invocations_count));
        }
    }

    if let Some(ref e) = engram_report {
        if e.status == ComplianceStatus::Violation {
            has_violation = true;
            violation_reasons.push(format!("engram-summary: missing summary for {}", e.target_date));
        }
    }

    if let Some(ref v) = vg_report {
        if v.status == ComplianceStatus::Violation {
            has_violation = true;
            violation_reasons.push("vg-sync: graph is stale compared to vault HEAD".to_string());
        }
    }

    let status = if has_violation {
        ComplianceStatus::Violation
    } else {
        ComplianceStatus::Ok
    };

    let exit_code = if has_violation { 1 } else { 0 };

    let summary = if has_violation {
        format!("VIOLATION: {}", violation_reasons.join(", "))
    } else {
        "OK: all enabled compliance checks passed".to_string()
    };

    Ok(ComplianceReport {
        schema_version: 1,
        agent: args.agent,
        status,
        exit_code,
        checks: ComplianceChecks {
            rtk_usage: rtk_report,
            engram_summary: engram_report,
            vg_sync: vg_report,
        },
        summary,
    })
}

// ---------------------------------------------------------------------------
// 1. RTK-USAGE CHECK
// ---------------------------------------------------------------------------

pub fn check_rtk_usage(log_path_str: Option<&str>) -> Result<RtkUsageReport> {
    let resolved_path = log_path_str
        .map(PathBuf::from)
        .or_else(|| std::env::var("ORQ_COMPLIANCE_LOG").ok().map(PathBuf::from))
        .or_else(|| std::env::var("ORQ_AGENT_LOG").ok().map(PathBuf::from));

    let Some(path) = resolved_path else {
        return Ok(RtkUsageReport {
            status: ComplianceStatus::NotAvailable,
            raw_invocations_count: 0,
            scanned_files_count: 0,
            violations: Vec::new(),
            message: "no log disponible".to_string(),
        });
    };

    if !path.exists() {
        return Ok(RtkUsageReport {
            status: ComplianceStatus::NotAvailable,
            raw_invocations_count: 0,
            scanned_files_count: 0,
            violations: Vec::new(),
            message: format!("log path not found: {}", path.display()),
        });
    }

    let mut files_to_scan = Vec::new();
    if path.is_file() {
        files_to_scan.push(path);
    } else if path.is_dir() {
        collect_files_recursively(&path, &mut files_to_scan)?;
    }

    let scanned_files_count = files_to_scan.len();
    let mut violations = Vec::new();

    for file_path in &files_to_scan {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;
            let line_trimmed = line.trim();
            if line_trimmed.is_empty() {
                continue;
            }

            // If line is JSON (e.g. transcript.jsonl or hook input), extract command string
            if let Some(cmd_str) = extract_command_from_json(line_trimmed) {
                scan_command_string(&cmd_str, file_path, line_num, &mut violations);
            } else {
                scan_command_string(line_trimmed, file_path, line_num, &mut violations);
            }
        }
    }

    let raw_invocations_count = violations.len();
    let status = if raw_invocations_count > 0 {
        ComplianceStatus::Violation
    } else {
        ComplianceStatus::Ok
    };

    let message = if raw_invocations_count > 0 {
        format!(
            "detected {} raw invocation(s) without rtk in {} file(s)",
            raw_invocations_count, scanned_files_count
        )
    } else {
        format!(
            "all commands properly wrapped with rtk across {} file(s)",
            scanned_files_count
        )
    };

    Ok(RtkUsageReport {
        status,
        raw_invocations_count,
        scanned_files_count,
        violations,
        message,
    })
}

fn collect_files_recursively(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let _ = collect_files_recursively(&p, files);
            } else if p.is_file() {
                files.push(p);
            }
        }
    }
    Ok(())
}

fn extract_command_from_json(line: &str) -> Option<String> {
    if !line.starts_with('{') || !line.ends_with('}') {
        return None;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
        return None;
    };

    if let Some(cmd) = v.get("command").and_then(|c| c.as_str()) {
        return Some(cmd.to_string());
    }
    if let Some(cmd) = v.get("CommandLine").and_then(|c| c.as_str()) {
        return Some(cmd.to_string());
    }
    if let Some(cmd) = v
        .get("tool_input")
        .and_then(|t| t.get("command"))
        .and_then(|c| c.as_str())
    {
        return Some(cmd.to_string());
    }
    if let Some(cmd) = v
        .get("tool_input")
        .and_then(|t| t.get("CommandLine"))
        .and_then(|c| c.as_str())
    {
        return Some(cmd.to_string());
    }
    None
}

const RAW_BINARIES: &[&str] = &[
    "git", "find", "ls", "rg", "grep", "fd", "egrep", "fgrep", "ag", "ack",
];

fn scan_command_string(
    cmd: &str,
    file_path: &Path,
    line_num: usize,
    violations: &mut Vec<RtkViolation>,
) {
    if cmd.contains("RTK_TEXTO_LIBRE=1") {
        return;
    }

    // Split segments by chain operators
    let segments = split_command_segments(cmd);
    for seg in segments {
        let seg_trimmed = seg.trim();
        if seg_trimmed.is_empty() {
            continue;
        }

        let tokens = tokenize_segment(seg_trimmed);
        if tokens.is_empty() {
            continue;
        }

        // Skip leading environment variable assignments: FOO=bar
        let mut idx = 0;
        while idx < tokens.len() {
            let tok = tokens[idx];
            if is_env_var_assignment(tok) {
                idx += 1;
            } else {
                break;
            }
        }

        if idx >= tokens.len() {
            continue;
        }

        // Skip sudo if present
        if tokens[idx] == "sudo" {
            idx += 1;
            // Also skip sudo options like -u user or -E
            while idx < tokens.len() && tokens[idx].starts_with('-') {
                idx += 1;
                // If flag takes argument like -u <user>, skip argument too
                if idx < tokens.len() && !tokens[idx].starts_with('-') && tokens[idx - 1] == "-u" {
                    idx += 1;
                }
            }
        }

        if idx >= tokens.len() {
            continue;
        }

        let base_binary = extract_binary_name(tokens[idx]);

        // If the binary is rtk, then this command is safely wrapped
        if base_binary == "rtk" {
            continue;
        }

        // If the binary matches a raw prohibited command
        if RAW_BINARIES.contains(&base_binary.as_str()) {
            violations.push(RtkViolation {
                file: file_path.display().to_string(),
                line: line_num,
                raw_command: seg_trimmed.to_string(),
                binary: base_binary,
            });
        }
    }
}

fn split_command_segments(cmd: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut current_start = 0;
    let bytes = cmd.as_bytes();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' if !in_double_quote => in_single_quote = !in_single_quote,
            b'"' if !in_single_quote => in_double_quote = !in_double_quote,
            b';' if !in_single_quote && !in_double_quote => {
                segments.push(&cmd[current_start..i]);
                current_start = i + 1;
            }
            b'&' if !in_single_quote && !in_double_quote && i + 1 < bytes.len() && bytes[i + 1] == b'&' => {
                segments.push(&cmd[current_start..i]);
                i += 1;
                current_start = i + 1;
            }
            b'|' if !in_single_quote && !in_double_quote => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    segments.push(&cmd[current_start..i]);
                    i += 1;
                    current_start = i + 1;
                } else {
                    segments.push(&cmd[current_start..i]);
                    current_start = i + 1;
                }
            }
            b'\n' if !in_single_quote && !in_double_quote => {
                segments.push(&cmd[current_start..i]);
                current_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if current_start < cmd.len() {
        segments.push(&cmd[current_start..]);
    }
    segments
}

fn tokenize_segment(seg: &str) -> Vec<&str> {
    seg.split_whitespace().collect()
}

fn is_env_var_assignment(token: &str) -> bool {
    if let Some((k, _)) = token.split_once('=') {
        !k.is_empty()
            && k.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
            && k.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
    } else {
        false
    }
}

fn extract_binary_name(token: &str) -> String {
    let clean = token.trim_matches(|c| c == '\'' || c == '"');
    let path = Path::new(clean);
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(clean)
        .to_string()
}

// ---------------------------------------------------------------------------
// 2. ENGRAM-SUMMARY CHECK
// ---------------------------------------------------------------------------

pub async fn check_engram_summary(
    project_str: Option<&str>,
    engram_bin_str: Option<&str>,
) -> Result<EngramSummaryReport> {
    let project = project_str
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ORQ_PROJECT").ok())
        .or_else(|| std::env::var("ENGRAM_PROJECT").ok())
        .unwrap_or_else(detect_default_project);

    let engram_bin = engram_bin_str
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ORQ_ENGRAM_BIN").ok())
        .unwrap_or_else(|| "engram".to_string());

    let target_date = get_today_date_string();

    // Verify binary exists
    let bin_path_resolved = which::which(&engram_bin);
    if bin_path_resolved.is_err() && !Path::new(&engram_bin).exists() {
        return Ok(EngramSummaryReport {
            status: ComplianceStatus::NotAvailable,
            project,
            target_date,
            session_summaries_count: 0,
            engram_binary: engram_bin,
            message: "engram no disponible".to_string(),
        });
    }

    let output_res = tokio::process::Command::new(&engram_bin)
        .args(["search", "session_summary", "--project", &project])
        .output()
        .await;

    let output = match output_res {
        Ok(out) => out,
        Err(err) => {
            return Ok(EngramSummaryReport {
                status: ComplianceStatus::NotAvailable,
                project,
                target_date,
                session_summaries_count: 0,
                engram_binary: engram_bin,
                message: format!("engram error: {}", err),
            });
        }
    };

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let session_summaries_count = count_today_session_summaries(&stdout_str, &target_date);

    let (status, message) = if session_summaries_count > 0 {
        (
            ComplianceStatus::Ok,
            format!(
                "found {} session_summary for today ({}) in project '{}'",
                session_summaries_count, target_date, project
            ),
        )
    } else {
        (
            ComplianceStatus::Violation,
            format!(
                "no session_summary found for today ({}) in project '{}'",
                target_date, project
            ),
        )
    };

    Ok(EngramSummaryReport {
        status,
        project,
        target_date,
        session_summaries_count,
        engram_binary: engram_bin,
        message,
    })
}

fn detect_default_project() -> String {
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(name) = cwd.file_name().and_then(|n| n.to_str()) {
            return name.to_string();
        }
    }
    "agent-orchestrator".to_string()
}

fn get_today_date_string() -> String {
    #[cfg(unix)]
    {
        let now = unsafe { libc::time(std::ptr::null_mut()) };
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        unsafe { libc::localtime_r(&now, &mut tm) };
        format!(
            "{:04}-{:02}-{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday
        )
    }
    #[cfg(not(unix))]
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = now / 86400;
        // Simple fallback
        format!("day-{}", days)
    }
}

fn count_today_session_summaries(output: &str, target_date: &str) -> usize {
    let mut count = 0;
    let lines: Vec<&str> = output.lines().collect();
    let mut current_is_session_summary = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.contains("(session_summary)") {
            current_is_session_summary = true;
        } else if trimmed.starts_with('[') && trimmed.contains('(') {
            current_is_session_summary = false;
        }

        if current_is_session_summary && trimmed.contains(target_date) {
            count += 1;
            current_is_session_summary = false;
        }
    }

    count
}

// ---------------------------------------------------------------------------
// 3. VG-SYNC CHECK
// ---------------------------------------------------------------------------

pub fn check_vg_sync(
    vault_path_str: Option<&str>,
    kuzu_path_str: Option<&str>,
) -> Result<VgSyncReport> {
    let vault_path = vault_path_str
        .map(PathBuf::from)
        .or_else(|| std::env::var("ORQ_VAULT_PATH").ok().map(PathBuf::from))
        .or_else(|| std::env::var("VAULT_PATH").ok().map(PathBuf::from));

    let kuzu_path = kuzu_path_str
        .map(PathBuf::from)
        .or_else(|| std::env::var("ORQ_KUZU_PATH").ok().map(PathBuf::from))
        .or_else(|| std::env::var("KUZU_PATH").ok().map(PathBuf::from));

    let (Some(vault_path), Some(kuzu_path)) = (vault_path.as_ref(), kuzu_path.as_ref()) else {
        return Ok(VgSyncReport {
            status: ComplianceStatus::NotAvailable,
            vault_path: vault_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            kuzu_path: kuzu_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            vault_head_unix: None,
            kuzu_sync_unix: None,
            is_fresh: false,
            message: "vault o kuzu path no disponible".to_string(),
        });
    };

    let vault_head_unix = get_vault_head_unix(vault_path);
    let kuzu_sync_unix = get_kuzu_sync_unix(kuzu_path);

    let (status, is_fresh, message) = match (vault_head_unix, kuzu_sync_unix) {
        (Some(head), Some(kuzu)) => {
            if kuzu >= head {
                (
                    ComplianceStatus::Ok,
                    true,
                    format!("kuzu graph is fresh (sync: {} >= vault HEAD: {})", kuzu, head),
                )
            } else {
                (
                    ComplianceStatus::Violation,
                    false,
                    format!(
                        "kuzu graph is stale (sync: {} < vault HEAD: {}, lag: {}s)",
                        kuzu,
                        head,
                        head - kuzu
                    ),
                )
            }
        }
        (None, _) => (
            ComplianceStatus::Violation,
            false,
            format!("unable to obtain vault git HEAD timestamp at {}", vault_path.display()),
        ),
        (_, None) => (
            ComplianceStatus::Violation,
            false,
            format!("kuzu graph not found or timestamp unavailable at {}", kuzu_path.display()),
        ),
    };

    Ok(VgSyncReport {
        status,
        vault_path: vault_path.display().to_string(),
        kuzu_path: kuzu_path.display().to_string(),
        vault_head_unix,
        kuzu_sync_unix,
        is_fresh,
        message,
    })
}

fn get_vault_head_unix(vault_path: &Path) -> Option<u64> {
    if !vault_path.exists() {
        return None;
    }

    let git_dir = if vault_path.join(".git").is_dir() {
        vault_path.join(".git")
    } else if vault_path.join(".git").is_file() {
        if let Ok(content) = fs::read_to_string(vault_path.join(".git")) {
            if let Some(target) = content.trim().strip_prefix("gitdir:") {
                let target_path = PathBuf::from(target.trim());
                if target_path.is_absolute() {
                    target_path
                } else {
                    vault_path.join(target_path)
                }
            } else {
                vault_path.join(".git")
            }
        } else {
            vault_path.join(".git")
        }
    } else if vault_path.join("HEAD").exists() {
        vault_path.to_path_buf()
    } else {
        vault_path.join(".git")
    };

    // 1. Try resolving current branch from .git/HEAD
    let head_file = git_dir.join("HEAD");
    if let Ok(head_content) = fs::read_to_string(&head_file) {
        let trimmed = head_content.trim();
        if let Some(ref_rel) = trimmed.strip_prefix("ref:") {
            let branch_file = git_dir.join(ref_rel.trim());
            if branch_file.exists() {
                if let Ok(meta) = fs::metadata(&branch_file) {
                    if let Ok(mtime) = meta.modified() {
                        return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
                    }
                }
            }
        }
    }

    // Direct check for main or master
    let main_ref = git_dir.join("refs/heads/main");
    if main_ref.exists() {
        if let Ok(meta) = fs::metadata(&main_ref) {
            if let Ok(mtime) = meta.modified() {
                return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
            }
        }
    }

    let master_ref = git_dir.join("refs/heads/master");
    if master_ref.exists() {
        if let Ok(meta) = fs::metadata(&master_ref) {
            if let Ok(mtime) = meta.modified() {
                return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
            }
        }
    }

    // Check any ref in refs/heads/
    let refs_heads = git_dir.join("refs/heads");
    if refs_heads.is_dir() {
        if let Ok(entries) = fs::read_dir(&refs_heads) {
            let mut latest_ref_mtime = 0u64;
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if let Ok(secs) = mtime.duration_since(UNIX_EPOCH).map(|d| d.as_secs()) {
                            if secs > latest_ref_mtime {
                                latest_ref_mtime = secs;
                            }
                        }
                    }
                }
            }
            if latest_ref_mtime > 0 {
                return Some(latest_ref_mtime);
            }
        }
    }

    // Fallback 1: .git/FETCH_HEAD
    let fetch_head = git_dir.join("FETCH_HEAD");
    if fetch_head.exists() {
        if let Ok(meta) = fs::metadata(&fetch_head) {
            if let Ok(mtime) = meta.modified() {
                return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
            }
        }
    }

    // Fallback 2: .git/index
    let index = git_dir.join("index");
    if index.exists() {
        if let Ok(meta) = fs::metadata(&index) {
            if let Ok(mtime) = meta.modified() {
                return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
            }
        }
    }

    // Fallback 3: .git/HEAD
    if head_file.exists() {
        if let Ok(meta) = fs::metadata(&head_file) {
            if let Ok(mtime) = meta.modified() {
                return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
            }
        }
    }

    // Fallback 4: vault_path directory itself
    if let Ok(meta) = fs::metadata(vault_path) {
        if let Ok(mtime) = meta.modified() {
            return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
        }
    }

    None
}

fn get_kuzu_sync_unix(kuzu_path: &Path) -> Option<u64> {
    if !kuzu_path.exists() {
        return None;
    }

    // If there is a sync marker file (e.g. kuzu_path.sync)
    let marker = kuzu_path.with_extension("sync");
    if marker.exists() {
        if let Ok(meta) = fs::metadata(&marker) {
            if let Ok(mtime) = meta.modified() {
                return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
            }
        }
    }

    if kuzu_path.is_file() {
        let meta = fs::metadata(kuzu_path).ok()?;
        let mtime = meta.modified().ok()?;
        return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
    }

    if kuzu_path.is_dir() {
        let mut max_mtime = 0u64;
        if let Ok(entries) = fs::read_dir(kuzu_path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if let Ok(secs) = mtime.duration_since(UNIX_EPOCH).map(|d| d.as_secs()) {
                            if secs > max_mtime {
                                max_mtime = secs;
                            }
                        }
                    }
                }
            }
        }
        if max_mtime > 0 {
            return Some(max_mtime);
        }
        let meta = fs::metadata(kuzu_path).ok()?;
        let mtime = meta.modified().ok()?;
        return mtime.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtk_scanner_detects_raw_git_and_find() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_file = temp_dir.path().join("session.log");
        fs::write(
            &log_file,
            "rtk git status\ngit status\nrtk find . -name '*.rs'\nfind /tmp\n",
        )
        .unwrap();

        let report = check_rtk_usage(Some(log_file.to_str().unwrap())).unwrap();
        assert_eq!(report.status, ComplianceStatus::Violation);
        assert_eq!(report.raw_invocations_count, 2);
        assert_eq!(report.violations.len(), 2);
        assert_eq!(report.violations[0].binary, "git");
        assert_eq!(report.violations[0].line, 2);
        assert_eq!(report.violations[1].binary, "find");
        assert_eq!(report.violations[1].line, 4);
    }

    #[test]
    fn test_rtk_scanner_accepts_clean_rtk_log() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_file = temp_dir.path().join("clean.log");
        fs::write(
            &log_file,
            "rtk git status\nrtk proxy git status\nrtk ls -la\nrtk rg foo\n",
        )
        .unwrap();

        let report = check_rtk_usage(Some(log_file.to_str().unwrap())).unwrap();
        assert_eq!(report.status, ComplianceStatus::Ok);
        assert_eq!(report.raw_invocations_count, 0);
    }

    #[test]
    fn test_rtk_scanner_honors_escape() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_file = temp_dir.path().join("escape.log");
        fs::write(&log_file, "RTK_TEXTO_LIBRE=1 git log -p\n").unwrap();

        let report = check_rtk_usage(Some(log_file.to_str().unwrap())).unwrap();
        assert_eq!(report.status, ComplianceStatus::Ok);
        assert_eq!(report.raw_invocations_count, 0);
    }

    #[test]
    fn test_count_today_session_summaries() {
        let output = r#"
Found 2 memories:

[1] #42 (session_summary) — Session summary: agent-orchestrator
    ## Goal
    Work on task
    2026-09-04 17:33:10 | project: agent-orchestrator | scope: project

[2] #40 (decision) — Decision: something
    2026-09-04 17:28:04 | project: agent-orchestrator | scope: project
"#;
        assert_eq!(count_today_session_summaries(output, "2026-09-04"), 1);
        assert_eq!(count_today_session_summaries(output, "2026-09-03"), 0);
    }

    #[test]
    fn test_vg_sync_fresh_and_stale() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vault_dir = temp_dir.path().join("vault");
        let git_refs = vault_dir.join(".git/refs/heads");
        fs::create_dir_all(&git_refs).unwrap();
        fs::write(vault_dir.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        let main_ref = git_refs.join("main");
        fs::write(&main_ref, "commit1\n").unwrap();

        let kuzu_file = temp_dir.path().join("vault.kuzu");
        fs::write(&kuzu_file, "kuzu db content\n").unwrap();

        // When kuzu is newer or same
        let report_fresh = check_vg_sync(
            Some(vault_dir.to_str().unwrap()),
            Some(kuzu_file.to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(report_fresh.status, ComplianceStatus::Ok);
        assert!(report_fresh.is_fresh);

        // When vault ref is newer than kuzu
        // Set kuzu file mtime to 10 seconds ago
        let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(10);
        let times = std::fs::FileTimes::new().set_modified(old_time);
        let file = std::fs::File::options().write(true).open(&kuzu_file).unwrap();
        let _ = file.set_times(times);

        let report_stale = check_vg_sync(
            Some(vault_dir.to_str().unwrap()),
            Some(kuzu_file.to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(report_stale.status, ComplianceStatus::Violation);
        assert!(!report_stale.is_fresh);
    }

    #[test]
    fn test_vg_sync_not_available_when_no_paths() {
        let report = check_vg_sync(None, None).unwrap();
        if std::env::var("ORQ_VAULT_PATH").is_err() && std::env::var("VAULT_PATH").is_err() {
            assert_eq!(report.status, ComplianceStatus::NotAvailable);
            assert!(!report.is_fresh);
        }
    }
}
