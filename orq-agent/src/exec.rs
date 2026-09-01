use crate::adapters::{find_adapter_in_registry, AdaptersRegistry};
use crate::policy;
use crate::receipt::{now_unix, tail_sanitized, ExecReceipt, ExecStatus};
use color_eyre::eyre::Result;
use std::collections::VecDeque;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time;

const OUTPUT_TAIL_BYTES: usize = 16 * 1024;
const MAX_TASK_BYTES: usize = 1024 * 1024;
const MAX_TIMEOUT_SECONDS: u64 = 300;

#[derive(Debug)]
pub struct ExecRequest {
    pub agent: String,
    pub model: String,
    pub task_file: String,
    pub timeout_seconds: u64,
    pub allow_gated: bool,
    pub correlation_id: Option<String>,
    pub policy_config: policy::PolicyConfig,
    pub adapters_registry: AdaptersRegistry,
}

pub async fn run(request: ExecRequest) -> Result<ExecReceipt> {
    let started_at_unix = now_unix();
    let started = Instant::now();
    let correlation_id = request
        .correlation_id
        .clone()
        .unwrap_or_else(|| format!("orq-agent-{}", started_at_unix));

    if request.timeout_seconds == 0 || request.timeout_seconds > MAX_TIMEOUT_SECONDS {
        return Ok(invalid_receipt(
            &request,
            correlation_id,
            started_at_unix,
            started,
            format!(
                "timeout must be between 1 and {} seconds",
                MAX_TIMEOUT_SECONDS
            ),
        ));
    }

    let Some(adapter) = find_adapter_in_registry(&request.agent, &request.adapters_registry) else {
        return Ok(invalid_receipt(
            &request,
            correlation_id,
            started_at_unix,
            started,
            format!("unknown agent adapter: {}", request.agent),
        ));
    };

    let policy = policy::evaluate(
        adapter.name(),
        &request.model,
        adapter.status(),
        request.allow_gated,
        &request.policy_config,
    );

    if !policy.allowed {
        return Ok(ExecReceipt {
            schema_version: 1,
            correlation_id,
            agent: request.agent,
            model: request.model,
            command: Vec::new(),
            status: ExecStatus::Blocked,
            policy_reason: policy.reason,
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request.timeout_seconds,
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
        });
    }

    let task_bytes = match tokio::fs::read(&request.task_file).await {
        Ok(task_bytes) => task_bytes,
        Err(error) => {
            return Ok(invalid_receipt(
                &request,
                correlation_id,
                started_at_unix,
                started,
                format!("reading task file {}: {error}", request.task_file),
            ));
        }
    };
    if task_bytes.len() > MAX_TASK_BYTES {
        return Ok(invalid_receipt(
            &request,
            correlation_id,
            started_at_unix,
            started,
            format!(
                "task file {} exceeds {} bytes",
                request.task_file, MAX_TASK_BYTES
            ),
        ));
    }
    let task = match String::from_utf8(task_bytes) {
        Ok(task) => task,
        Err(error) => {
            return Ok(invalid_receipt(
                &request,
                correlation_id,
                started_at_unix,
                started,
                format!(
                    "task file {} is not valid UTF-8: {error}",
                    request.task_file
                ),
            ));
        }
    };

    let argv = adapter.build_argv(&request.model, &task);
    let binary = adapter
        .binary_path()
        .unwrap_or_else(|| adapter.binary().to_string());
    let mut command_for_receipt = vec![binary.clone()];
    command_for_receipt.extend(argv.iter().map(|arg| {
        if arg == &task {
            format!("<task {} bytes>", task.len())
        } else {
            arg.clone()
        }
    }));

    let mut command = Command::new(&binary);
    command
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_process_group(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ok(ExecReceipt {
                schema_version: 1,
                correlation_id,
                agent: request.agent,
                model: request.model,
                command: command_for_receipt,
                status: ExecStatus::SpawnFailed,
                policy_reason: policy.reason,
                started_at_unix,
                duration_ms: started.elapsed().as_millis(),
                timeout_seconds: request.timeout_seconds,
                exit_code: None,
                stdout_tail: String::new(),
                stderr_tail: format!("spawning agent {} via {}: {error}", adapter.name(), binary),
                secrets_read: false,
            });
        }
    };

    let child_id = child.id();
    let stdout_task = child
        .stdout
        .take()
        .map(|stdout| tokio::spawn(read_tail(stdout, OUTPUT_TAIL_BYTES)));
    let stderr_task = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(read_tail(stderr, OUTPUT_TAIL_BYTES)));

    let wait_result =
        time::timeout(Duration::from_secs(request.timeout_seconds), child.wait()).await;

    let (status, exit_code, timeout_message) = match wait_result {
        Ok(Ok(status)) => (
            if status.success() {
                ExecStatus::Succeeded
            } else {
                ExecStatus::Failed
            },
            status.code(),
            None,
        ),
        Ok(Err(error)) => (ExecStatus::SpawnFailed, None, Some(error.to_string())),
        Err(_) => {
            let kill_warning = kill_process_group(child_id);
            let _ = time::timeout(Duration::from_secs(2), child.kill()).await;
            let _ = time::timeout(Duration::from_secs(2), child.wait()).await;
            let mut message = format!("timed out after {} seconds", request.timeout_seconds);
            if let Some(warning) = kill_warning {
                message.push_str("; ");
                message.push_str(&warning);
            }
            (ExecStatus::TimedOut, None, Some(message))
        }
    };

    let stdout_tail = collect_tail(stdout_task).await;
    let mut stderr_tail = collect_tail(stderr_task).await;
    if let Some(message) = timeout_message {
        let mut stderr_bytes = stderr_tail.into_bytes();
        if !stderr_bytes.is_empty() {
            stderr_bytes.push(b'\n');
        }
        stderr_bytes.extend_from_slice(message.as_bytes());
        stderr_tail = tail_sanitized(&stderr_bytes, OUTPUT_TAIL_BYTES);
    }

    Ok(ExecReceipt {
        schema_version: 1,
        correlation_id,
        agent: request.agent,
        model: request.model,
        command: command_for_receipt,
        status,
        policy_reason: policy.reason,
        started_at_unix,
        duration_ms: started.elapsed().as_millis(),
        timeout_seconds: request.timeout_seconds,
        exit_code,
        stdout_tail,
        stderr_tail,
        secrets_read: false,
    })
}

async fn read_tail<R>(mut reader: R, max_bytes: usize) -> Vec<u8>
where
    R: AsyncRead + Unpin,
{
    let mut ring = VecDeque::with_capacity(max_bytes);
    let mut buffer = [0_u8; 4096];

    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(read) => {
                for byte in &buffer[..read] {
                    if ring.len() == max_bytes {
                        ring.pop_front();
                    }
                    ring.push_back(*byte);
                }
            }
            Err(error) => {
                let message = format!("[orq-agent output read error: {error}]");
                for byte in message.as_bytes() {
                    if ring.len() == max_bytes {
                        ring.pop_front();
                    }
                    ring.push_back(*byte);
                }
                break;
            }
        }
    }

    ring.into_iter().collect()
}

async fn collect_tail(task: Option<tokio::task::JoinHandle<Vec<u8>>>) -> String {
    let Some(task) = task else {
        return String::new();
    };

    match time::timeout(Duration::from_secs(2), task).await {
        Ok(Ok(bytes)) => tail_sanitized(&bytes, OUTPUT_TAIL_BYTES),
        Ok(Err(error)) => format!("[orq-agent output task join error: {error}]"),
        Err(_) => "[orq-agent output collection timed out]".to_string(),
    }
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn kill_process_group(child_id: Option<u32>) -> Option<String> {
    let pid = child_id?;
    let result = unsafe { libc::kill(-(pid as libc::pid_t), libc::SIGKILL) };
    if result == 0 {
        None
    } else {
        Some(format!(
            "kill process group {} failed: {}",
            pid,
            std::io::Error::last_os_error()
        ))
    }
}

#[cfg(not(unix))]
fn kill_process_group(_child_id: Option<u32>) -> Option<String> {
    Some("process group kill is unsupported on this platform".to_string())
}

fn invalid_receipt(
    request: &ExecRequest,
    correlation_id: String,
    started_at_unix: u64,
    started: Instant,
    reason: String,
) -> ExecReceipt {
    ExecReceipt {
        schema_version: 1,
        correlation_id,
        agent: request.agent.clone(),
        model: request.model.clone(),
        command: Vec::new(),
        status: ExecStatus::InvalidRequest,
        policy_reason: reason,
        started_at_unix,
        duration_ms: started.elapsed().as_millis(),
        timeout_seconds: request.timeout_seconds,
        exit_code: None,
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        secrets_read: false,
    }
}
