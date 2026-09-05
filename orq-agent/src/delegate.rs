use crate::adapters::{find_adapter_in_registry, AdaptersRegistry};
use crate::policy;
use crate::receipt::{
    now_unix, now_unix_nanos, tail_sanitized, DelegateReceipt, DelegateStatus, DelegateVerdict,
};
use color_eyre::eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time;

const OUTPUT_TAIL_BYTES: usize = 16 * 1024;
const MAX_TIMEOUT_SECONDS: u64 = 600;

#[derive(Debug, Clone)]
pub struct DelegateRequest {
    pub task: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub handoff: Option<String>,
    pub repo_path: Option<String>,
    pub agents_dir: Option<String>,
    pub workspace: Option<String>,
    pub write_handoff: Option<String>,
    pub write_receipt: Option<String>,
    pub force: bool,
    pub execute: bool,
    pub timeout_seconds: u64,
    pub correlation_id: Option<String>,
    pub policy_config: policy::PolicyConfig,
    pub adapters_registry: AdaptersRegistry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateOutput {
    pub status: DelegateStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub verdict: DelegateVerdict,
    pub evidence: String,
    pub agent: String,
    pub model: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autonomous_command: Option<String>,
    pub next_step: String,
    pub must_stop_for_delegation: bool,
    pub supervisor_only: bool,
    pub execution_agent_allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_handoff: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_receipt: Option<String>,
    pub receipt: DelegateReceipt,
}

pub async fn run(request: DelegateRequest) -> Result<DelegateOutput> {
    let started_at_unix = now_unix();
    let started = Instant::now();
    let correlation_id = request
        .correlation_id
        .clone()
        .unwrap_or_else(|| format!("orq-delegate-{}-{}", now_unix_nanos(), std::process::id()));

    let target_agent = request.agent.clone().unwrap_or_else(|| "agy".to_string());
    let target_model = request
        .model
        .clone()
        .unwrap_or_else(|| default_model_for_agent(&target_agent));

    let task_text = build_task_text(&request);
    let prompt = build_prompt(&task_text, &target_agent, &target_model);
    let auto_cmd = build_autonomous_command(&request, &target_agent, &target_model, &task_text);

    let is_pi = is_pi_agent(&target_agent);
    let supervisor_only = is_pi;
    let must_stop = is_pi && !request.execute;
    let exec_allowed = !must_stop;

    let repo_dir = request
        .repo_path
        .as_deref()
        .or(request.workspace.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    if !request.execute {
        // Plan / Command Generated mode
        let status = if auto_cmd.is_some() {
            DelegateStatus::CommandGenerated
        } else {
            DelegateStatus::Planned
        };
        let next_step = if must_stop {
            "ejecutar el comando sugerido en el agente externo y volver con recibo verificable"
                .to_string()
        } else {
            "delegacion planificada; ejecutar con --execute para correr el agente externo"
                .to_string()
        };

        let receipt = DelegateReceipt {
            schema_version: 1,
            correlation_id: correlation_id.clone(),
            agent: target_agent.clone(),
            model: target_model.clone(),
            command: auto_cmd.clone().map(|c| vec![c]).unwrap_or_default(),
            status: status.clone(),
            reason: None,
            verdict: DelegateVerdict::Indeterminado,
            evidence: "none".to_string(),
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request.timeout_seconds,
            exit_code: None,
            secrets_read: false,
        };

        let mut output = DelegateOutput {
            status,
            reason: None,
            verdict: DelegateVerdict::Indeterminado,
            evidence: "none".to_string(),
            agent: target_agent,
            model: target_model,
            prompt,
            command: auto_cmd.clone(),
            autonomous_command: auto_cmd,
            next_step,
            must_stop_for_delegation: must_stop,
            supervisor_only,
            execution_agent_allowed: exec_allowed,
            written_handoff: None,
            written_receipt: None,
            receipt,
        };

        write_delegation_artifacts(&request, &mut output).await?;
        return Ok(output);
    }

    // Execution mode: verify git pre-state
    let pre_head = get_git_head(&repo_dir).await;
    let pre_branch = get_git_branch(&repo_dir).await;

    // Check adapter
    let adapter_opt = find_adapter_in_registry(&target_agent, &request.adapters_registry);
    let timeout_secs = request.timeout_seconds.clamp(1, MAX_TIMEOUT_SECONDS);

    let (exit_code, stdout, stderr, timed_out, command_vec) = if let Some(adapter) = adapter_opt {
        let binary = adapter
            .binary_path()
            .unwrap_or_else(|| adapter.binary().to_string());
        let argv = adapter.build_argv(&target_model, &task_text);
        let mut cmd_for_receipt = vec![binary.clone()];
        cmd_for_receipt.extend(argv.clone());

        let mut cmd = Command::new(&binary);
        cmd.args(&argv)
            .current_dir(&repo_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        run_process(cmd, timeout_secs, cmd_for_receipt).await?
    } else {
        // Fallback: spawn bash command directly
        let cmd_str = auto_cmd
            .clone()
            .unwrap_or_else(|| format!("rtk {}", target_agent));
        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(&cmd_str)
            .current_dir(&repo_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        run_process(
            cmd,
            timeout_secs,
            vec!["bash".to_string(), "-c".to_string(), cmd_str],
        )
        .await?
    };

    // Post-execution git inspection
    let post_head = get_git_head(&repo_dir).await;
    let post_branch = get_git_branch(&repo_dir).await;

    let has_new_commit = match (&pre_head, &post_head) {
        (Some(pre), Some(post)) => pre != post,
        (None, Some(_)) => true,
        _ => false,
    };
    let has_new_branch = match (&pre_branch, &post_branch) {
        (Some(pre), Some(post)) => pre != post,
        (None, Some(_)) => true,
        _ => false,
    };
    let extracted_ref = extract_pr_or_ref_from_text(&stdout);

    let (status, reason, verdict, evidence) = if timed_out {
        if has_new_commit {
            let commit_hash = post_head.clone().unwrap_or_else(|| "none".to_string());
            (
                DelegateStatus::Executed,
                Some("timeout_con_evidencia".to_string()),
                DelegateVerdict::Util,
                commit_hash,
            )
        } else {
            (
                DelegateStatus::Failed,
                Some("timeout_sin_evidencia".to_string()),
                DelegateVerdict::NonUtil,
                "none".to_string(),
            )
        }
    } else if exit_code == Some(0) || exit_code.is_none() {
        if has_new_commit {
            let commit_hash = post_head.clone().unwrap_or_else(|| "none".to_string());
            (
                DelegateStatus::Validated,
                None,
                DelegateVerdict::Util,
                commit_hash,
            )
        } else if has_new_branch {
            let branch_ref = format!("branch:{}", post_branch.as_deref().unwrap_or("unknown"));
            (
                DelegateStatus::Validated,
                None,
                DelegateVerdict::Util,
                branch_ref,
            )
        } else if let Some(ref_str) = extracted_ref {
            (
                DelegateStatus::Validated,
                None,
                DelegateVerdict::Util,
                ref_str,
            )
        } else {
            (
                DelegateStatus::Failed,
                Some("no_executed".to_string()),
                DelegateVerdict::NonUtil,
                "none".to_string(),
            )
        }
    } else if has_new_commit {
        let commit_hash = post_head.clone().unwrap_or_else(|| "none".to_string());
        (
            DelegateStatus::Executed,
            Some("exit_non_zero_with_commit".to_string()),
            DelegateVerdict::Util,
            commit_hash,
        )
    } else {
        (
            DelegateStatus::Failed,
            Some("no_executed".to_string()),
            DelegateVerdict::NonUtil,
            "none".to_string(),
        )
    };

    let next_step = match status {
        DelegateStatus::Validated => {
            "delegacion validada exitosamente con evidencia de trabajo".to_string()
        }
        DelegateStatus::Executed => {
            "delegacion ejecutada en background; verificar estado final antes de cerrar".to_string()
        }
        DelegateStatus::Failed => format!(
            "delegacion fallida (reason={}); revisar logs o reintentar",
            reason.as_deref().unwrap_or("unknown")
        ),
        _ => "verificar estado de delegacion".to_string(),
    };

    let receipt = DelegateReceipt {
        schema_version: 1,
        correlation_id: correlation_id.clone(),
        agent: target_agent.clone(),
        model: target_model.clone(),
        command: command_vec,
        status: status.clone(),
        reason: reason.clone(),
        verdict: verdict.clone(),
        evidence: evidence.clone(),
        stdout_tail: stdout,
        stderr_tail: stderr,
        started_at_unix,
        duration_ms: started.elapsed().as_millis(),
        timeout_seconds: timeout_secs,
        exit_code,
        secrets_read: false,
    };

    let mut output = DelegateOutput {
        status,
        reason,
        verdict,
        evidence,
        agent: target_agent,
        model: target_model,
        prompt,
        command: auto_cmd.clone(),
        autonomous_command: auto_cmd,
        next_step,
        must_stop_for_delegation: false,
        supervisor_only,
        execution_agent_allowed: true,
        written_handoff: None,
        written_receipt: None,
        receipt,
    };

    write_delegation_artifacts(&request, &mut output).await?;
    Ok(output)
}

async fn run_process(
    mut cmd: Command,
    timeout_secs: u64,
    command_vec: Vec<String>,
) -> Result<(Option<i32>, String, String, bool, Vec<String>)> {
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            return Ok((
                Some(1),
                String::new(),
                format!("spawn failed: {err}"),
                false,
                command_vec,
            ));
        }
    };

    let stdout_task = child
        .stdout
        .take()
        .map(|stdout| tokio::spawn(read_tail(stdout, OUTPUT_TAIL_BYTES)));
    let stderr_task = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(read_tail(stderr, OUTPUT_TAIL_BYTES)));

    let wait_result = time::timeout(Duration::from_secs(timeout_secs), child.wait()).await;

    let (exit_code, timed_out) = match wait_result {
        Ok(Ok(status)) => (status.code(), false),
        Ok(Err(_)) => (Some(1), false),
        Err(_) => {
            let _ = child.kill().await;
            (None, true)
        }
    };

    let stdout = match stdout_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    };
    let stderr = match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    };

    Ok((exit_code, stdout, stderr, timed_out, command_vec))
}

async fn read_tail<R: AsyncRead + Unpin>(mut reader: R, max_bytes: usize) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    while let Ok(n) = reader.read(&mut chunk).await {
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if buffer.len() > max_bytes * 2 {
            let start = buffer.len().saturating_sub(max_bytes);
            buffer = buffer[start..].to_vec();
        }
    }
    tail_sanitized(&buffer, max_bytes)
}

async fn get_git_head(repo_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

async fn get_git_branch(repo_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() && text != "HEAD" {
            return Some(text);
        }
    }
    None
}

fn extract_pr_or_ref_from_text(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(pos) = trimmed.find("https://github.com/") {
            let candidate = &trimmed[pos..];
            let url = candidate.split_whitespace().next().unwrap_or(candidate);
            if url.contains("/pull/") {
                return Some(url.to_string());
            }
        }
        if let Some(pos) = trimmed.find("PR #") {
            let candidate = &trimmed[pos..];
            let pr = candidate
                .split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ");
            return Some(pr);
        }
    }
    None
}

#[allow(dead_code)]
async fn get_git_status_count(repo_dir: &Path) -> usize {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_dir)
        .output()
        .await;

    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            return text.lines().filter(|l| !l.trim().is_empty()).count();
        }
    }
    0
}

#[allow(dead_code)]
fn is_plan_only_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.contains("plan de acción")
        || lower.contains("plan:")
        || lower.contains("here is the plan")
        || lower.contains("pasos propuestos")
        || lower.contains("¿desea continuar?")
        || lower.contains("approval required")
        || lower.contains("no he realizado cambios")
        || lower.contains("propuesta de cambios"))
        && !lower.contains("commit:")
        && !lower.contains("committed")
}

fn build_task_text(request: &DelegateRequest) -> String {
    if let Some(ref t) = request.task {
        if !t.trim().is_empty() {
            return t.trim().to_string();
        }
    }
    if let Some(ref h) = request.handoff {
        return format!(
            "ejecutar handoff {}",
            Path::new(h)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(h)
        );
    }
    if let Some(ref wh) = request.write_handoff {
        return format!(
            "ejecutar handoff {}",
            Path::new(wh)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(wh)
        );
    }
    "tarea delegada".to_string()
}

fn default_model_for_agent(agent: &str) -> String {
    let norm = agent.to_ascii_lowercase();
    if norm.contains("agy") || norm.contains("antigravity") {
        "gemini-3.7-flash-high".to_string()
    } else if norm.contains("claude") {
        "claude-sonnet-5".to_string()
    } else if norm.contains("hermes") {
        "deepseek-v4-flash".to_string()
    } else if norm.contains("openclaw") {
        "openclaw-main".to_string()
    } else {
        "gpt-4o-mini".to_string()
    }
}

fn is_pi_agent(agent: &str) -> bool {
    let norm = agent.to_ascii_lowercase();
    norm == "pi" || norm == "pi-api" || norm.starts_with("pi/")
}

fn build_prompt(task: &str, agent: &str, model: &str) -> String {
    format!(
        "OBLIGATORIO: Usa rtk; todo comando de terminal/git/filesystem debe ir prefijado con rtk.\n\
         Tarea: {}\n\
         Delegado a agente: {} | modelo: {}\n\
         Al terminar, reporta comandos de validacion y estado final con recibo verificable.",
        task, agent, model
    )
}

fn build_autonomous_command(
    request: &DelegateRequest,
    agent: &str,
    model: &str,
    task: &str,
) -> Option<String> {
    let norm = agent.to_ascii_lowercase();
    let workspace = request
        .workspace
        .as_deref()
        .unwrap_or("/home/freddy/Workspace");
    let repo = request
        .repo_path
        .as_deref()
        .unwrap_or("/home/freddy/Workspace/Desarrollo/agent-orchestrator");
    let agents_dir = request
        .agents_dir
        .as_deref()
        .unwrap_or("/home/freddy/Workspace/.agents");

    let handoff_target = request
        .handoff
        .as_deref()
        .or(request.write_handoff.as_deref());
    let print_instruction = if let Some(h) = handoff_target {
        format!("Olvida el historial anterior. Lee y ejecuta {}", h)
    } else if task.ends_with(".md") || task.contains("handoffs/") {
        format!("Olvida el historial anterior. Lee y ejecuta {}", task)
    } else {
        format!("Olvida el historial anterior. {}", task)
    };

    if norm.contains("agy") || norm.contains("antigravity") {
        Some(format!(
            "cd {}\nrtk agy --model {} --dangerously-skip-permissions --add-dir {} --add-dir {} --print={:?}",
            workspace, model, repo, agents_dir, print_instruction
        ))
    } else if norm.contains("hermes") {
        Some(format!(
            "cd {}\nrtk hermes -m {} -z {:?}",
            workspace, model, print_instruction
        ))
    } else if norm.contains("openclaw") {
        Some(format!(
            "rtk openclaw agent --agent main --model {} --message {:?}",
            model, print_instruction
        ))
    } else {
        None
    }
}

async fn write_delegation_artifacts(
    request: &DelegateRequest,
    output: &mut DelegateOutput,
) -> Result<()> {
    if let Some(ref handoff_path) = request.write_handoff {
        let content = format!(
            "# HANDOFF: {}\n\n**Fecha (unix):** {}\n**Agente:** {}\n**Modelo:** {}\n\n## Tarea\n{}\n",
            build_task_text(request),
            now_unix(),
            output.agent,
            output.model,
            output.prompt
        );
        write_file_safely(handoff_path, content.as_bytes(), request.force).await?;
        output.written_handoff = Some(handoff_path.clone());
    }

    if let Some(ref receipt_path) = request.write_receipt {
        let json_data =
            serde_json::to_string_pretty(&output.receipt).wrap_err("serialize receipt")?;
        write_file_safely(receipt_path, json_data.as_bytes(), request.force).await?;
        output.written_receipt = Some(receipt_path.clone());
    }

    Ok(())
}

async fn write_file_safely(path: &str, data: &[u8], force: bool) -> Result<()> {
    let clean_path = Path::new(path);
    if !force && clean_path.exists() {
        color_eyre::eyre::bail!(
            "file already exists at '{}' (use --force to overwrite)",
            path
        );
    }
    if let Some(parent) = clean_path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    tokio::fs::write(clean_path, data).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AdaptersRegistry;
    use crate::policy::PolicyConfig;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn setup_git_repo(dir: &Path) {
        let run_cmd = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(dir)
                .status()
                .expect("run git command");
            assert!(status.success());
        };
        run_cmd(&["init"]);
        run_cmd(&["config", "user.name", "Freddy Taborda"]);
        run_cmd(&["config", "user.email", "terracenter@gmail.com"]);
        fs::write(dir.join("README.md"), "initial").unwrap();
        run_cmd(&["add", "README.md"]);
        run_cmd(&["commit", "-m", "initial commit"]);
    }

    fn make_executable_script(dir: &Path, name: &str, content: &str) -> PathBuf {
        let script_path = dir.join(name);
        fs::write(&script_path, content).unwrap();
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
        script_path
    }

    fn test_policy_config() -> PolicyConfig {
        PolicyConfig {
            schema_version: 1,
            approval_required_model_patterns: vec![],
            blocked_adapter_statuses: vec![],
            gated_adapter_statuses: vec![],
        }
    }

    fn test_empty_adapters_registry() -> AdaptersRegistry {
        AdaptersRegistry {
            schema_version: 1,
            adapters: vec![],
        }
    }

    fn test_adapters_registry(agent_name: &str, script_path: &Path) -> AdaptersRegistry {
        let json = format!(
            r#"{{"schema_version":1,"adapters":[{{"name":"{}","binary":"{}","status":"available","argv":["$MODEL","$TASK"]}}]}}"#,
            agent_name,
            script_path.display()
        );
        serde_json::from_str(&json).unwrap()
    }

    #[tokio::test]
    async fn test_state_planned_transition() {
        let request = DelegateRequest {
            task: Some("dummy task".to_string()),
            agent: Some("custom-no-auto-cmd".to_string()),
            model: Some("custom-model".to_string()),
            handoff: None,
            repo_path: None,
            agents_dir: None,
            workspace: None,
            write_handoff: None,
            write_receipt: None,
            force: false,
            execute: false,
            timeout_seconds: 10,
            correlation_id: Some("corr-plan-1".to_string()),
            policy_config: test_policy_config(),
            adapters_registry: test_empty_adapters_registry(),
        };

        let output = run(request).await.expect("run delegate");
        assert_eq!(output.status, DelegateStatus::Planned);
        assert_eq!(output.verdict, DelegateVerdict::Indeterminado);
        assert_eq!(output.evidence, "none");
        assert!(output.command.is_none());
        assert_eq!(output.receipt.status, DelegateStatus::Planned);
    }

    #[tokio::test]
    async fn test_state_command_generated_transition() {
        let request = DelegateRequest {
            task: Some("corregir bug".to_string()),
            agent: Some("agy".to_string()),
            model: Some("gemini-3.7-flash-high".to_string()),
            handoff: None,
            repo_path: None,
            agents_dir: None,
            workspace: None,
            write_handoff: None,
            write_receipt: None,
            force: false,
            execute: false,
            timeout_seconds: 10,
            correlation_id: Some("corr-cmd-gen-1".to_string()),
            policy_config: test_policy_config(),
            adapters_registry: test_empty_adapters_registry(),
        };

        let output = run(request).await.expect("run delegate");
        assert_eq!(output.status, DelegateStatus::CommandGenerated);
        assert_eq!(output.verdict, DelegateVerdict::Indeterminado);
        assert_eq!(output.evidence, "none");
        assert!(output.command.is_some());
        assert_eq!(output.receipt.status, DelegateStatus::CommandGenerated);
    }

    #[tokio::test]
    async fn test_state_validated_with_commit() {
        let temp = tempdir().unwrap();
        setup_git_repo(temp.path());

        let script = make_executable_script(
            temp.path(),
            "runner_commit.sh",
            "#!/usr/bin/env bash\necho 'change' >> README.md\ngit add README.md\ngit commit -m 'new fix'\n",
        );
        let registry = test_adapters_registry("test-agent", &script);

        let request = DelegateRequest {
            task: Some("fix file".to_string()),
            agent: Some("test-agent".to_string()),
            model: Some("test-model".to_string()),
            handoff: None,
            repo_path: Some(temp.path().display().to_string()),
            agents_dir: None,
            workspace: None,
            write_handoff: None,
            write_receipt: None,
            force: false,
            execute: true,
            timeout_seconds: 10,
            correlation_id: Some("corr-val-1".to_string()),
            policy_config: test_policy_config(),
            adapters_registry: registry,
        };

        let output = run(request).await.expect("run delegate");
        assert_eq!(output.status, DelegateStatus::Validated);
        assert_eq!(output.verdict, DelegateVerdict::Util);
        assert_ne!(output.evidence, "none");
        assert_ne!(output.evidence, "worktree_changes");
        assert_eq!(output.reason, None);
        assert_eq!(output.receipt.status, DelegateStatus::Validated);
    }

    #[tokio::test]
    async fn test_state_validated_with_new_branch() {
        let temp = tempdir().unwrap();
        setup_git_repo(temp.path());

        let script = make_executable_script(
            temp.path(),
            "runner_branch.sh",
            "#!/usr/bin/env bash\ngit checkout -b feature-test-branch\n",
        );
        let registry = test_adapters_registry("test-agent", &script);

        let request = DelegateRequest {
            task: Some("create branch".to_string()),
            agent: Some("test-agent".to_string()),
            model: Some("test-model".to_string()),
            handoff: None,
            repo_path: Some(temp.path().display().to_string()),
            agents_dir: None,
            workspace: None,
            write_handoff: None,
            write_receipt: None,
            force: false,
            execute: true,
            timeout_seconds: 10,
            correlation_id: Some("corr-val-branch-1".to_string()),
            policy_config: test_policy_config(),
            adapters_registry: registry,
        };

        let output = run(request).await.expect("run delegate");
        assert_eq!(output.status, DelegateStatus::Validated);
        assert_eq!(output.verdict, DelegateVerdict::Util);
        assert_eq!(output.evidence, "branch:feature-test-branch");
        assert_eq!(output.reason, None);
    }

    #[tokio::test]
    async fn test_state_failed_plan_only_detection() {
        let temp = tempdir().unwrap();
        setup_git_repo(temp.path());

        let script = make_executable_script(
            temp.path(),
            "runner_plan_only.sh",
            "#!/usr/bin/env bash\necho 'Plan de acción:'\necho '1. Modificar archivo'\necho '¿Desea continuar?'\n",
        );
        let registry = test_adapters_registry("test-agent", &script);

        let request = DelegateRequest {
            task: Some("plan task".to_string()),
            agent: Some("test-agent".to_string()),
            model: Some("test-model".to_string()),
            handoff: None,
            repo_path: Some(temp.path().display().to_string()),
            agents_dir: None,
            workspace: None,
            write_handoff: None,
            write_receipt: None,
            force: false,
            execute: true,
            timeout_seconds: 10,
            correlation_id: Some("corr-plan-only-1".to_string()),
            policy_config: test_policy_config(),
            adapters_registry: registry,
        };

        let output = run(request).await.expect("run delegate");
        assert_eq!(output.status, DelegateStatus::Failed);
        assert_eq!(output.verdict, DelegateVerdict::NonUtil);
        assert_eq!(output.reason, Some("no_executed".to_string()));
        assert_eq!(output.evidence, "none");
    }

    #[tokio::test]
    async fn test_state_failed_timeout_without_evidence() {
        let temp = tempdir().unwrap();
        setup_git_repo(temp.path());

        let script = make_executable_script(
            temp.path(),
            "runner_timeout_no_ev.sh",
            "#!/usr/bin/env bash\nsleep 3\n",
        );
        let registry = test_adapters_registry("test-agent", &script);

        let request = DelegateRequest {
            task: Some("slow task".to_string()),
            agent: Some("test-agent".to_string()),
            model: Some("test-model".to_string()),
            handoff: None,
            repo_path: Some(temp.path().display().to_string()),
            agents_dir: None,
            workspace: None,
            write_handoff: None,
            write_receipt: None,
            force: false,
            execute: true,
            timeout_seconds: 1,
            correlation_id: Some("corr-timeout-no-ev".to_string()),
            policy_config: test_policy_config(),
            adapters_registry: registry,
        };

        let output = run(request).await.expect("run delegate");
        assert_eq!(output.status, DelegateStatus::Failed);
        assert_eq!(output.verdict, DelegateVerdict::NonUtil);
        assert_eq!(output.reason, Some("timeout_sin_evidencia".to_string()));
        assert_eq!(output.evidence, "none");
    }

    #[tokio::test]
    async fn test_state_executed_timeout_with_commit() {
        let temp = tempdir().unwrap();
        setup_git_repo(temp.path());

        let script = make_executable_script(
            temp.path(),
            "runner_timeout_with_commit.sh",
            "#!/usr/bin/env bash\necho 'partial' >> README.md\ngit add README.md\ngit commit -m 'partial work'\nsleep 3\n",
        );
        let registry = test_adapters_registry("test-agent", &script);

        let request = DelegateRequest {
            task: Some("slow task with commit".to_string()),
            agent: Some("test-agent".to_string()),
            model: Some("test-model".to_string()),
            handoff: None,
            repo_path: Some(temp.path().display().to_string()),
            agents_dir: None,
            workspace: None,
            write_handoff: None,
            write_receipt: None,
            force: false,
            execute: true,
            timeout_seconds: 1,
            correlation_id: Some("corr-timeout-commit".to_string()),
            policy_config: test_policy_config(),
            adapters_registry: registry,
        };

        let output = run(request).await.expect("run delegate");
        assert_eq!(output.status, DelegateStatus::Executed);
        assert_eq!(output.verdict, DelegateVerdict::Util);
        assert_eq!(output.reason, Some("timeout_con_evidencia".to_string()));
        assert_ne!(output.evidence, "none");
    }

    #[tokio::test]
    async fn test_git_verification_exit_zero_without_evidence() {
        let temp = tempdir().unwrap();
        setup_git_repo(temp.path());

        // Command exits 0 but does not create commit, branch or change
        let script = make_executable_script(
            temp.path(),
            "runner_exit_zero_no_changes.sh",
            "#!/usr/bin/env bash\necho 'all good but no real changes done'\nexit 0\n",
        );
        let registry = test_adapters_registry("test-agent", &script);

        let request = DelegateRequest {
            task: Some("noop task".to_string()),
            agent: Some("test-agent".to_string()),
            model: Some("test-model".to_string()),
            handoff: None,
            repo_path: Some(temp.path().display().to_string()),
            agents_dir: None,
            workspace: None,
            write_handoff: None,
            write_receipt: None,
            force: false,
            execute: true,
            timeout_seconds: 10,
            correlation_id: Some("corr-exit-zero-no-ev".to_string()),
            policy_config: test_policy_config(),
            adapters_registry: registry,
        };

        let output = run(request).await.expect("run delegate");
        assert_eq!(output.status, DelegateStatus::Failed);
        assert_eq!(output.verdict, DelegateVerdict::NonUtil);
        assert_eq!(output.reason, Some("no_executed".to_string()));
        assert_eq!(output.evidence, "none");
    }
}
