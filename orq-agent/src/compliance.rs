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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pi_supervision: Option<PiSupervisionReport>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PiLawCheck {
    pub law: String,
    pub status: ComplianceStatus,
    pub evidence: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PiSupervisionReport {
    pub status: ComplianceStatus,
    pub checks: Vec<PiLawCheck>,
    pub summary: String,
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
    pub db_path: Option<String>,
    pub agents_path: Option<String>,
    pub handoffs_path: Option<String>,
}

pub async fn run_compliance(args: ComplianceArgs) -> Result<ComplianceReport> {
    let is_pi = args.agent.as_deref() == Some("pi");
    let has_explicit_check = args.rtk_usage || args.engram_summary || args.vg_sync;
    let run_all = !has_explicit_check && args.agent.is_none();

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

    let pi_report = if is_pi {
        Some(
            check_pi_supervision(
                args.agents_path.as_deref(),
                args.handoffs_path.as_deref(),
                args.db_path.as_deref(),
            )
            .await?,
        )
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

    if let Some(ref p) = pi_report {
        if p.status == ComplianceStatus::Violation {
            has_violation = true;
            violation_reasons.push(format!("pi-supervision: {}", p.summary));
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
            pi_supervision: pi_report,
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

// ---------------------------------------------------------------------------
// 4. PI SUPERVISION CHECK (6 LAWS)
// ---------------------------------------------------------------------------

pub async fn check_pi_supervision(
    agents_path_str: Option<&str>,
    handoffs_path_str: Option<&str>,
    db_path_str: Option<&str>,
) -> Result<PiSupervisionReport> {
    let agents_path = resolve_agents_path(agents_path_str);
    let handoffs_path = resolve_handoffs_path(handoffs_path_str, &agents_path);

    let law1 = check_pi_law1_guardia(&agents_path).await?;
    let law2 = check_pi_law2_documented(&agents_path)?;
    let law3 = check_pi_law3_separation_of_duties(db_path_str)?;
    let law4 = check_pi_law4_routing_delegation(&handoffs_path)?;
    let law5 = check_pi_law5_plan_criteria(&handoffs_path)?;
    let law6 = check_pi_law6_no_self_review(db_path_str, &handoffs_path)?;

    let checks = vec![law1, law2, law3, law4, law5, law6];
    let mut violations = Vec::new();

    for check in &checks {
        if check.status == ComplianceStatus::Violation {
            violations.push(format!("{}: {}", check.law, check.message));
        }
    }

    let (status, summary) = if !violations.is_empty() {
        (
            ComplianceStatus::Violation,
            format!("{} violation(s) detected: {}", violations.len(), violations.join("; ")),
        )
    } else {
        (
            ComplianceStatus::Ok,
            "all 6 Pi supervision laws passed or not applicable".to_string(),
        )
    };

    Ok(PiSupervisionReport {
        status,
        checks,
        summary,
    })
}

fn resolve_agents_path(custom: Option<&str>) -> PathBuf {
    if let Some(p) = custom {
        return PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("ORQ_AGENTS_PATH") {
        return PathBuf::from(p);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home).join(".agents");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(".agents")
}

fn resolve_handoffs_path(custom: Option<&str>, agents_path: &Path) -> PathBuf {
    if let Some(p) = custom {
        return PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("ORQ_HANDOFFS_PATH") {
        return PathBuf::from(p);
    }
    let p = agents_path.join("handoffs");
    if p.exists() {
        return p;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let hp = PathBuf::from(home).join(".agents").join("handoffs");
        if hp.exists() {
            return hp;
        }
    }
    p
}

fn find_file_in_candidates(candidates: &[PathBuf]) -> Option<PathBuf> {
    for p in candidates {
        if p.is_file() {
            return Some(p.clone());
        }
    }
    None
}

// Law 1: guardia-pi instalada y pasando
pub async fn check_pi_law1_guardia(agents_path: &Path) -> Result<PiLawCheck> {
    let law = "Ley 1: guardia-pi instalada y pasando".to_string();

    let guardia_candidates = [
        agents_path.join("scripts/guardia-pi.sh"),
        agents_path.join("guardia-pi.sh"),
    ];
    let test_candidates = [
        agents_path.join("scripts/test-guardia-pi.sh"),
        agents_path.join("test-guardia-pi.sh"),
    ];

    let guardia_path = find_file_in_candidates(&guardia_candidates);
    let test_path = find_file_in_candidates(&test_candidates);

    let (Some(guardia_path), Some(test_path)) = (guardia_path, test_path) else {
        return Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence: format!(
                "guardia_candidates: {:?}, test_candidates: {:?}",
                guardia_candidates, test_candidates
            ),
            message: "guardia-pi.sh o test-guardia-pi.sh no encontrados en el directorio de agents"
                .to_string(),
        });
    };

    let output = match tokio::process::Command::new("bash")
        .arg(&test_path)
        .env("AGENTS_DIR", agents_path)
        .output()
        .await
    {
        Ok(out) => out,
        Err(err) => {
            return Ok(PiLawCheck {
                law,
                status: ComplianceStatus::Violation,
                evidence: format!("exec error: {}", err),
                message: format!("error al ejecutar test-guardia-pi.sh: {}", err),
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Ok,
            evidence: format!(
                "guardia: {}, test: {}, salida: {}",
                guardia_path.display(),
                test_path.display(),
                stdout.lines().last().unwrap_or("ok").trim()
            ),
            message: "guardia-pi.sh y test-guardia-pi.sh instalados y pasando exit 0".to_string(),
        })
    } else {
        let code = output.status.code().unwrap_or(1);
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence: format!(
                "exit: {}, stdout: {}, stderr: {}",
                code,
                stdout.trim(),
                stderr.trim()
            ),
            message: format!("test-guardia-pi.sh falló con exit code {}", code),
        })
    }
}

// Law 2: leyes documentadas
pub fn check_pi_law2_documented(agents_path: &Path) -> Result<PiLawCheck> {
    let law = "Ley 2: leyes documentadas".to_string();

    let pi_md_candidates = [
        agents_path.join("orquestadores/pi.md"),
        agents_path.join("pi.md"),
    ];

    let Some(pi_md_path) = find_file_in_candidates(&pi_md_candidates) else {
        return Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence: format!("candidates: {:?}", pi_md_candidates),
            message: "orquestadores/pi.md no encontrado".to_string(),
        });
    };

    let content = match fs::read_to_string(&pi_md_path) {
        Ok(c) => c,
        Err(err) => {
            return Ok(PiLawCheck {
                law,
                status: ComplianceStatus::Violation,
                evidence: format!("read error: {}", err),
                message: format!("no se pudo leer {}: {}", pi_md_path.display(), err),
            });
        }
    };

    let content_lower = content.to_lowercase();
    let mut detected_laws = Vec::new();

    // 1. Backlog
    if content_lower.contains("backlog")
        && (content_lower.contains("leer")
            || content_lower.contains("leído")
            || content_lower.contains("leido"))
    {
        detected_laws.push("1. Leer el backlog completo antes de priorizar");
    } else if content_lower.contains("ley 1") || content_lower.contains("### 1") {
        detected_laws.push("1. Backlog");
    }

    // 2. Rankear por valor/urgencia/dependencia
    if content_lower.contains("valor")
        && content_lower.contains("urgencia")
        && content_lower.contains("dependencia")
    {
        detected_laws.push("2. Rankear por valor/urgencia/dependencia");
    } else if content_lower.contains("ley 2") || content_lower.contains("### 2") {
        detected_laws.push("2. Priorización");
    }

    // 3. Macro antes que micro
    if (content_lower.contains("macro") && content_lower.contains("micro"))
        || content_lower.contains("ley 3")
        || content_lower.contains("### 3")
    {
        detected_laws.push("3. Macro antes que micro");
    }

    // 4. Revisor distinto del implementador
    if (content_lower.contains("revisor") && content_lower.contains("implementador"))
        || content_lower.contains("review_agent")
    {
        detected_laws.push("4. Revisor distinto del implementador");
    } else if content_lower.contains("ley 4") || content_lower.contains("### 4") {
        detected_laws.push("4. Segregación de roles");
    }

    // 5. Delegación vía routing
    if content_lower.contains("orq route")
        || content_lower.contains("orq delegate")
        || content_lower.contains("enrutado adaptativo")
    {
        detected_laws.push("5. Delegar vía orq route/delegate (enrutado adaptativo)");
    } else if content_lower.contains("ley 5") || content_lower.contains("### 5") {
        detected_laws.push("5. Routing");
    }

    // 6. Revisión oficial 4R
    if content_lower.contains("orq review 4r")
        || (content_lower.contains("review") && content_lower.contains("4r"))
    {
        detected_laws.push("6. Usar orq review 4r del orquestador");
    } else if content_lower.contains("ley 6") || content_lower.contains("### 6") {
        detected_laws.push("6. Review 4R");
    }

    if detected_laws.len() == 6 {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Ok,
            evidence: format!(
                "path: {}, 6 leyes detectadas: {:?}",
                pi_md_path.display(),
                detected_laws
            ),
            message: "orquestadores/pi.md contiene las 6 leyes de supervisión".to_string(),
        })
    } else {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence: format!(
                "path: {}, detectadas {}/6 leyes: {:?}",
                pi_md_path.display(),
                detected_laws.len(),
                detected_laws
            ),
            message: format!(
                "orquestadores/pi.md incompleto (detectadas {}/6 leyes)",
                detected_laws.len()
            ),
        })
    }
}

// Law 3: separación de deberes (revisor ≠ implementador)
pub fn check_pi_law3_separation_of_duties(db_path_str: Option<&str>) -> Result<PiLawCheck> {
    let law = "Ley 3: separación de deberes (revisor ≠ implementador)".to_string();

    let path = db_path_str.map(Path::new);
    let store = match crate::state::open(path) {
        Ok(s) => s,
        Err(err) => {
            return Ok(PiLawCheck {
                law,
                status: ComplianceStatus::NotAvailable,
                evidence: format!("state error: {}", err),
                message: format!("base de datos de estado no disponible: {}", err),
            });
        }
    };

    let receipts = match store.list_delegate_receipts() {
        Ok(r) => r,
        Err(err) => {
            return Ok(PiLawCheck {
                law,
                status: ComplianceStatus::NotAvailable,
                evidence: format!("query error: {}", err),
                message: format!("error al listar delegate receipts: {}", err),
            });
        }
    };

    let valid_receipts: Vec<_> = receipts
        .into_iter()
        .filter(|r| {
            matches!(
                r.status,
                crate::receipt::DelegateStatus::Validated
                    | crate::receipt::DelegateStatus::Executed
            ) || matches!(
                r.verdict,
                crate::receipt::DelegateVerdict::Util
                    | crate::receipt::DelegateVerdict::NonUtil
            )
        })
        .collect();

    if valid_receipts.is_empty() {
        return Ok(PiLawCheck {
            law,
            status: ComplianceStatus::NotAvailable,
            evidence: "[]".to_string(),
            message: "no hay delegate receipts en la base de datos para auditar".to_string(),
        });
    }

    let mut agents = std::collections::BTreeSet::new();
    let mut agent_models = std::collections::BTreeSet::new();

    for r in &valid_receipts {
        agents.insert(r.agent.clone());
        agent_models.insert(format!("{}:{}", r.agent, r.model));
    }

    let evidence_str = format!("{:?}", agent_models.into_iter().collect::<Vec<_>>());

    if agents.len() >= 2 {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Ok,
            evidence: evidence_str,
            message: format!("encontrados {} agentes distintos en los receipts", agents.len()),
        })
    } else {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence: evidence_str,
            message: format!(
                "todos los receipts ({}) provienen de un único agente ({:?}) - riesgo de auto-revisión",
                valid_receipts.len(),
                agents
            ),
        })
    }
}

// Law 4: delegación vía routing, sin hardcode
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardcodedModelViolation {
    pub file: String,
    pub line: usize,
    pub raw_line: String,
}

pub fn check_pi_law4_routing_delegation(handoffs_path: &Path) -> Result<PiLawCheck> {
    let law = "Ley 4: delegación vía routing, sin hardcode".to_string();

    if !handoffs_path.exists() {
        return Ok(PiLawCheck {
            law,
            status: ComplianceStatus::NotAvailable,
            evidence: format!("path: {}", handoffs_path.display()),
            message: "directorio de handoffs no existe".to_string(),
        });
    }

    let mut md_files = Vec::new();
    if let Ok(entries) = fs::read_dir(handoffs_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md") {
                md_files.push(p);
            }
        }
    }

    if md_files.is_empty() {
        return Ok(PiLawCheck {
            law,
            status: ComplianceStatus::NotAvailable,
            evidence: format!("path: {}", handoffs_path.display()),
            message: "no hay archivos markdown en el directorio de handoffs".to_string(),
        });
    }

    md_files.sort();

    let mut violations: Vec<HardcodedModelViolation> = Vec::new();

    for file_path in &md_files {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (idx, line) in content.lines().enumerate() {
            let line_trimmed = line.trim();
            if line_trimmed.is_empty() {
                continue;
            }

            if line_trimmed.starts_with('#')
                || line_trimmed.starts_with("**Modelo")
                || line_trimmed.starts_with("* **Modelo")
            {
                continue;
            }

            if is_hardcoded_model_invocation(line_trimmed) {
                violations.push(HardcodedModelViolation {
                    file: file_path.display().to_string(),
                    line: idx + 1,
                    raw_line: line_trimmed.to_string(),
                });
            }
        }
    }

    if violations.is_empty() {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Ok,
            evidence: format!(
                "escaneados {} archivos de handoffs sin violaciones",
                md_files.len()
            ),
            message: "delegaciones auditadas sin modelo hardcodeado".to_string(),
        })
    } else {
        let evidence_json = serde_json::to_string(&violations).unwrap_or_default();
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence: evidence_json,
            message: format!(
                "detectadas {} violaciones de modelo hardcodeado en handoffs",
                violations.len()
            ),
        })
    }
}

fn is_hardcoded_model_invocation(line: &str) -> bool {
    let lower = line.to_lowercase();
    if lower.contains("ejemplo")
        || lower.contains("example")
        || lower.contains("--help")
        || lower.contains("criterio")
    {
        return false;
    }

    if (lower.contains("orq")
        || lower.contains("cargo run")
        || lower.contains("delegate")
        || lower.contains("exec"))
        && lower.contains("--model")
    {
        if let Some(pos) = line.find("--model") {
            let rest = line[pos + 7..].trim();
            if let Some(token) = rest.split_whitespace().next() {
                if !token.starts_with('<') && !token.starts_with('$') && !token.starts_with('{') {
                    return true;
                }
            }
        }
    }

    false
}

// Law 5: plan con valor/urgencia/dependencia
pub fn check_pi_law5_plan_criteria(handoffs_path: &Path) -> Result<PiLawCheck> {
    let law = "Ley 5: plan con valor/urgencia/dependencia".to_string();

    if !handoffs_path.exists() {
        return Ok(PiLawCheck {
            law,
            status: ComplianceStatus::NotAvailable,
            evidence: format!("path: {}", handoffs_path.display()),
            message: "directorio de handoffs no existe".to_string(),
        });
    }

    let mut plan_files = Vec::new();
    if let Ok(entries) = fs::read_dir(handoffs_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md") {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.contains("plan") || name.starts_with("orq-") {
                    plan_files.push(p);
                }
            }
        }
    }

    if plan_files.is_empty() {
        return Ok(PiLawCheck {
            law,
            status: ComplianceStatus::NotAvailable,
            evidence: format!("path: {}", handoffs_path.display()),
            message: "no se encontraron handoffs de plan para auditar".to_string(),
        });
    }

    plan_files.sort_by(|a, b| {
        let ma = fs::metadata(a).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH);
        let mb = fs::metadata(b).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH);
        mb.cmp(&ma)
    });

    let mut found_valid_plan = false;
    let mut detected_recency_violation = false;
    let mut checked_plans = Vec::new();

    for p in &plan_files {
        let content = match fs::read_to_string(p) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lower = content.to_lowercase();
        let fname = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let has_valor = lower.contains("valor");
        let has_urgencia = lower.contains("urgencia");
        let has_dependencia = lower.contains("dependencia");

        if has_valor && has_urgencia && has_dependencia {
            found_valid_plan = true;
            checked_plans.push(format!("{}: OK (valor, urgencia, dependencia)", fname));
            break;
        }

        if (lower.contains("recencia")
            || lower.contains("mas reciente")
            || lower.contains("más reciente"))
            && (!has_valor || !has_urgencia || !has_dependencia)
        {
            detected_recency_violation = true;
            checked_plans.push(format!(
                "{}: VIOLATION (priorización por recencia sin 3 criterios)",
                fname
            ));
        } else {
            checked_plans.push(format!("{}: sin criterios completos", fname));
        }
    }

    if found_valid_plan {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Ok,
            evidence: checked_plans.join("; "),
            message: "plan incluye criterios de Valor, Urgencia y Dependencia".to_string(),
        })
    } else if detected_recency_violation {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence: checked_plans.join("; "),
            message: "se detectó priorización por recencia sin criterios de Valor, Urgencia y Dependencia".to_string(),
        })
    } else {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence: checked_plans.join("; "),
            message: "ningún plan auditado contiene los 3 criterios (Valor, Urgencia, Dependencia)".to_string(),
        })
    }
}

// Law 6: sin auto-revisión
pub fn check_pi_law6_no_self_review(
    db_path_str: Option<&str>,
    handoffs_path: &Path,
) -> Result<PiLawCheck> {
    let law = "Ley 6: sin auto-revisión".to_string();

    let mut audited_pairs = Vec::new();
    let mut self_reviews = Vec::new();

    // 1. Audit SQLite delegate receipts
    let path = db_path_str.map(Path::new);
    if let Ok(store) = crate::state::open(path) {
        if let Ok(receipts) = store.list_delegate_receipts() {
            for r in &receipts {
                let cmd_str = r.command.join(" ").to_lowercase();
                let is_review = cmd_str.contains("review") || cmd_str.contains("4r");
                if is_review {
                    for other in &receipts {
                        if other.correlation_id == r.correlation_id
                            && other.agent == r.agent
                            && other.command != r.command
                        {
                            self_reviews.push((
                                r.agent.clone(),
                                r.agent.clone(),
                                format!("receipt:{}", r.correlation_id),
                            ));
                        } else if other.correlation_id == r.correlation_id
                            && other.agent != r.agent
                        {
                            audited_pairs.push((
                                other.agent.clone(),
                                r.agent.clone(),
                                format!("receipt:{}", r.correlation_id),
                            ));
                        }
                    }
                }
            }
        }
    }

    // 2. Audit handoff merge files (*merge*.md or *.json in handoffs_path)
    if handoffs_path.exists() {
        if let Ok(entries) = fs::read_dir(handoffs_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if p.is_file() && (fname.contains("merge") || fname.ends_with(".json")) {
                    if let Ok(content) = fs::read_to_string(&p) {
                        if fname.ends_with(".json") {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                                let rev = v.get("review_agent").and_then(|a| a.as_str());
                                let imp = v.get("implementer_agent").and_then(|a| a.as_str());
                                if let (Some(r), Some(i)) = (rev, imp) {
                                    if r.eq_ignore_ascii_case(i) {
                                        self_reviews
                                            .push((i.to_string(), r.to_string(), fname.to_string()));
                                    } else {
                                        audited_pairs
                                            .push((i.to_string(), r.to_string(), fname.to_string()));
                                    }
                                }
                            }
                        } else {
                            let (imp, rev) = extract_agents_from_handoff(&content);
                            if let (Some(i), Some(r)) = (imp, rev) {
                                if i.eq_ignore_ascii_case(&r) {
                                    self_reviews.push((i, r, fname.to_string()));
                                } else {
                                    audited_pairs.push((i, r, fname.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if !self_reviews.is_empty() {
        let evidence = self_reviews
            .iter()
            .map(|(i, r, src)| format!("{}: implementador='{}' == revisor='{}'", src, i, r))
            .collect::<Vec<_>>()
            .join("; ");
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Violation,
            evidence,
            message: format!(
                "detectada auto-revisión en {} caso(s)",
                self_reviews.len()
            ),
        })
    } else if !audited_pairs.is_empty() {
        let evidence = audited_pairs
            .iter()
            .map(|(i, r, src)| format!("{}: implementador='{}', revisor='{}'", src, i, r))
            .collect::<Vec<_>>()
            .join("; ");
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::Ok,
            evidence,
            message: format!(
                "auditados {} casos de revisión/merge, sin auto-revisión",
                audited_pairs.len()
            ),
        })
    } else {
        Ok(PiLawCheck {
            law,
            status: ComplianceStatus::NotAvailable,
            evidence: "[]".to_string(),
            message: "no se encontraron handoffs de merge o receipts de revisión para auditar"
                .to_string(),
        })
    }
}

fn extract_agents_from_handoff(content: &str) -> (Option<String>, Option<String>) {
    let mut implementer = None;
    let mut reviewer = None;

    for line in content.lines() {
        let lower = line.to_lowercase();
        if reviewer.is_none()
            && (lower.contains("revisor:")
                || lower.contains("**revisor:**")
                || lower.contains("review_agent"))
        {
            reviewer = extract_agent_name_from_line(line, "revisor");
        }
        if implementer.is_none()
            && (lower.contains("agente:")
                || lower.contains("**agente:**")
                || lower.contains("implementador:")
                || lower.contains("implementer_agent"))
        {
            implementer = extract_agent_name_from_line(line, "agente")
                .or_else(|| extract_agent_name_from_line(line, "implementador"))
                .or_else(|| extract_agent_name_from_line(line, "implementer_agent"));
        }
    }

    (implementer, reviewer)
}

fn extract_agent_name_from_line(line: &str, key: &str) -> Option<String> {
    let lower = line.to_lowercase();
    let pos = lower.find(key)?;
    let after = &line[pos + key.len()..];
    let after_clean = after.trim_start_matches(|c: char| c == ':' || c == '*' || c.is_whitespace());
    let token = after_clean
        .split(|c: char| c.is_whitespace() || c == '·' || c == '|' || c == ',')
        .next()?;
    let trimmed = token.trim_matches(|c: char| c == '*' || c == '`' || c == '\'' || c == '"');
    if !trimmed.is_empty() {
        Some(trimmed.to_lowercase())
    } else {
        None
    }
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

    // ─── Tests for Pi Supervision (6 Laws) ───────────────────────────────────

    #[tokio::test]
    async fn test_pi_law1_guardia_missing_and_passing() {
        let dir = tempfile::tempdir().unwrap();
        let agents_path = dir.path();

        // Missing scripts
        let check_missing = check_pi_law1_guardia(agents_path).await.unwrap();
        assert_eq!(check_missing.status, ComplianceStatus::Violation);

        // Created dummy scripts that exit 0
        let scripts = agents_path.join("scripts");
        fs::create_dir_all(&scripts).unwrap();
        fs::write(scripts.join("guardia-pi.sh"), "#!/usr/bin/env bash\nexit 0\n").unwrap();
        fs::write(scripts.join("test-guardia-pi.sh"), "#!/usr/bin/env bash\necho 'TODO OK'\nexit 0\n").unwrap();

        let check_ok = check_pi_law1_guardia(agents_path).await.unwrap();
        assert_eq!(check_ok.status, ComplianceStatus::Ok);

        // Failing test script
        fs::write(scripts.join("test-guardia-pi.sh"), "#!/usr/bin/env bash\nexit 1\n").unwrap();
        let check_fail = check_pi_law1_guardia(agents_path).await.unwrap();
        assert_eq!(check_fail.status, ComplianceStatus::Violation);
    }

    #[test]
    fn test_pi_law2_laws_documented() {
        let dir = tempfile::tempdir().unwrap();
        let agents_path = dir.path();

        // Missing file
        let check_missing = check_pi_law2_documented(agents_path).unwrap();
        assert_eq!(check_missing.status, ComplianceStatus::Violation);

        // Incomplete laws file
        let orq_dir = agents_path.join("orquestadores");
        fs::create_dir_all(&orq_dir).unwrap();
        let pi_md = orq_dir.join("pi.md");
        fs::write(&pi_md, "# Pi\n1. Leer backlog\n2. Priorizar valor urgencia dependencia\n").unwrap();
        let check_incomplete = check_pi_law2_documented(agents_path).unwrap();
        assert_eq!(check_incomplete.status, ComplianceStatus::Violation);

        // Complete 6 laws
        let complete_text = r#"
# Orquestador PI
1. Leer el backlog completo antes de priorizar (Backlog leído)
2. Rankear por valor/urgencia/dependencia
3. Macro antes que micro
4. Revisor de 4R distinto del implementador (review_agent != implementer_agent)
5. Delegar vía orq route / orq delegate (enrutado adaptativo)
6. Usar orq review 4r del orquestador
"#;
        fs::write(&pi_md, complete_text).unwrap();
        let check_complete = check_pi_law2_documented(agents_path).unwrap();
        assert_eq!(check_complete.status, ComplianceStatus::Ok);
    }

    #[test]
    fn test_pi_law3_separation_of_duties() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test_state.sqlite");
        let store = crate::state::open(Some(&db_path)).unwrap();

        // Empty DB -> NotAvailable
        let check_empty = check_pi_law3_separation_of_duties(Some(db_path.to_str().unwrap())).unwrap();
        assert_eq!(check_empty.status, ComplianceStatus::NotAvailable);

        // Single agent -> Violation
        let r1 = crate::receipt::DelegateReceipt {
            schema_version: 1,
            correlation_id: "c1".to_string(),
            agent: "agy".to_string(),
            model: "gemini-3.7-flash-high".to_string(),
            command: vec!["orq".to_string()],
            status: crate::receipt::DelegateStatus::Validated,
            reason: None,
            verdict: crate::receipt::DelegateVerdict::Util,
            evidence: "ev1".to_string(),
            stdout_tail: "".to_string(),
            stderr_tail: "".to_string(),
            started_at_unix: 1,
            duration_ms: 10,
            timeout_seconds: 30,
            exit_code: Some(0),
            secrets_read: false,
        };
        store.insert_delegate_receipt(&r1, "task1").unwrap();

        let check_single = check_pi_law3_separation_of_duties(Some(db_path.to_str().unwrap())).unwrap();
        assert_eq!(check_single.status, ComplianceStatus::Violation);

        // Add receipt with second distinct agent -> Ok
        let r2 = crate::receipt::DelegateReceipt {
            schema_version: 1,
            correlation_id: "c2".to_string(),
            agent: "hermes".to_string(),
            model: "qwen-2.5-coder".to_string(),
            command: vec!["orq".to_string()],
            status: crate::receipt::DelegateStatus::Validated,
            reason: None,
            verdict: crate::receipt::DelegateVerdict::Util,
            evidence: "ev2".to_string(),
            stdout_tail: "".to_string(),
            stderr_tail: "".to_string(),
            started_at_unix: 2,
            duration_ms: 10,
            timeout_seconds: 30,
            exit_code: Some(0),
            secrets_read: false,
        };
        store.insert_delegate_receipt(&r2, "task2").unwrap();

        let check_multi = check_pi_law3_separation_of_duties(Some(db_path.to_str().unwrap())).unwrap();
        assert_eq!(check_multi.status, ComplianceStatus::Ok);
    }

    #[test]
    fn test_pi_law4_routing_delegation() {
        let dir = tempfile::tempdir().unwrap();
        let handoffs_path = dir.path();

        // No handoffs -> NotAvailable
        let check_empty = check_pi_law4_routing_delegation(handoffs_path).unwrap();
        assert_eq!(check_empty.status, ComplianceStatus::NotAvailable);

        // Clean handoff -> Ok
        let clean_file = handoffs_path.join("clean.md");
        fs::write(&clean_file, "# Handoff\n**Modelo:** claude\nrtk orq delegate --task foo\n").unwrap();
        let check_clean = check_pi_law4_routing_delegation(handoffs_path).unwrap();
        assert_eq!(check_clean.status, ComplianceStatus::Ok);

        // Hardcoded model in command -> Violation
        let bad_file = handoffs_path.join("bad.md");
        fs::write(&bad_file, "# Handoff\norq-agent exec --model claude-3-5-sonnet --task-file /tmp/t.md\n").unwrap();
        let check_bad = check_pi_law4_routing_delegation(handoffs_path).unwrap();
        assert_eq!(check_bad.status, ComplianceStatus::Violation);
    }

    #[test]
    fn test_pi_law5_plan_criteria() {
        let dir = tempfile::tempdir().unwrap();
        let handoffs_path = dir.path();

        // No plans -> NotAvailable
        let check_empty = check_pi_law5_plan_criteria(handoffs_path).unwrap();
        assert_eq!(check_empty.status, ComplianceStatus::NotAvailable);

        // Plan with all 3 criteria -> Ok
        let plan_file = handoffs_path.join("orq-plan-test.md");
        fs::write(
            &plan_file,
            "# Plan\nCriterios de priorización: Valor técnico, Urgencia operativa y Dependencia arquitectónica.\n",
        )
        .unwrap();
        let check_ok = check_pi_law5_plan_criteria(handoffs_path).unwrap();
        assert_eq!(check_ok.status, ComplianceStatus::Ok);

        // Plan missing criteria -> Violation
        fs::write(&plan_file, "# Plan\nPriorizado por recencia de issues.\n").unwrap();
        let check_fail = check_pi_law5_plan_criteria(handoffs_path).unwrap();
        assert_eq!(check_fail.status, ComplianceStatus::Violation);
    }

    #[test]
    fn test_pi_law6_no_self_review() {
        let dir = tempfile::tempdir().unwrap();
        let handoffs_path = dir.path();

        // No merge handoffs -> NotAvailable
        let check_empty = check_pi_law6_no_self_review(None, handoffs_path).unwrap();
        assert_eq!(check_empty.status, ComplianceStatus::NotAvailable);

        // Valid merge (distinct reviewer) -> Ok
        let merge_ok = handoffs_path.join("orq-merge-10.md");
        fs::write(&merge_ok, "# Merge 10\n**Agente:** agy · **Revisor:** hermes\n").unwrap();
        let check_ok = check_pi_law6_no_self_review(None, handoffs_path).unwrap();
        assert_eq!(check_ok.status, ComplianceStatus::Ok);

        // Self-review (reviewer == agent) -> Violation
        let merge_bad = handoffs_path.join("orq-merge-11.md");
        fs::write(&merge_bad, "# Merge 11\n**Agente:** agy · **Revisor:** agy\n").unwrap();
        let check_bad = check_pi_law6_no_self_review(None, handoffs_path).unwrap();
        assert_eq!(check_bad.status, ComplianceStatus::Violation);
    }

    #[tokio::test]
    async fn test_pi_supervision_full_run_ok() {
        let dir = tempfile::tempdir().unwrap();
        let agents_dir = dir.path();
        let scripts = agents_dir.join("scripts");
        let orq_dir = agents_dir.join("orquestadores");
        let handoffs = agents_dir.join("handoffs");
        fs::create_dir_all(&scripts).unwrap();
        fs::create_dir_all(&orq_dir).unwrap();
        fs::create_dir_all(&handoffs).unwrap();

        // 1. Scripts
        fs::write(scripts.join("guardia-pi.sh"), "#!/usr/bin/env bash\nexit 0\n").unwrap();
        fs::write(scripts.join("test-guardia-pi.sh"), "#!/usr/bin/env bash\nexit 0\n").unwrap();

        // 2. Laws
        let complete_text = r#"
# Orquestador PI
1. Leer el backlog completo antes de priorizar (Backlog leído)
2. Rankear por valor/urgencia/dependencia
3. Macro antes que micro
4. Revisor de 4R distinto del implementador (review_agent != implementer_agent)
5. Delegar vía orq route / orq delegate (enrutado adaptativo)
6. Usar orq review 4r del orquestador
"#;
        fs::write(orq_dir.join("pi.md"), complete_text).unwrap();

        // 3. State DB with >= 2 agents
        let db_path = agents_dir.join("state.sqlite");
        let store = crate::state::open(Some(&db_path)).unwrap();
        let make_receipt = |id: &str, agent: &str| crate::receipt::DelegateReceipt {
            schema_version: 1,
            correlation_id: id.to_string(),
            agent: agent.to_string(),
            model: "model".to_string(),
            command: vec!["cmd".to_string()],
            status: crate::receipt::DelegateStatus::Validated,
            reason: None,
            verdict: crate::receipt::DelegateVerdict::Util,
            evidence: "ev".to_string(),
            stdout_tail: "".to_string(),
            stderr_tail: "".to_string(),
            started_at_unix: 1,
            duration_ms: 10,
            timeout_seconds: 30,
            exit_code: Some(0),
            secrets_read: false,
        };
        store.insert_delegate_receipt(&make_receipt("c1", "agy"), "task1").unwrap();
        store.insert_delegate_receipt(&make_receipt("c2", "hermes"), "task2").unwrap();

        // 4. Clean handoff (no hardcoded model)
        fs::write(handoffs.join("orq-clean.md"), "# Handoff\nrtk orq delegate --task foo\n").unwrap();

        // 5. Plan with 3 criteria
        fs::write(handoffs.join("orq-plan-1.md"), "# Plan\nValor, Urgencia, Dependencia\n").unwrap();

        // 6. Merge with distinct reviewer
        fs::write(handoffs.join("orq-merge-1.md"), "# Merge\n**Agente:** agy · **Revisor:** hermes\n").unwrap();

        let report = check_pi_supervision(
            Some(agents_dir.to_str().unwrap()),
            Some(handoffs.to_str().unwrap()),
            Some(db_path.to_str().unwrap()),
        )
        .await
        .unwrap();

        assert_eq!(report.status, ComplianceStatus::Ok);
        assert_eq!(report.checks.len(), 6);
        for c in &report.checks {
            assert_eq!(c.status, ComplianceStatus::Ok);
        }

        // Test run_compliance integration
        let comp_report = run_compliance(ComplianceArgs {
            agent: Some("pi".to_string()),
            agents_path: Some(agents_dir.to_str().unwrap().to_string()),
            handoffs_path: Some(handoffs.to_str().unwrap().to_string()),
            db_path: Some(db_path.to_str().unwrap().to_string()),
            rtk_usage: false,
            engram_summary: false,
            vg_sync: false,
            ..Default::default()
        })
        .await
        .unwrap();

        assert_eq!(comp_report.status, ComplianceStatus::Ok);
        assert_eq!(comp_report.exit_code, 0);
        assert!(comp_report.checks.pi_supervision.is_some());
    }
}

