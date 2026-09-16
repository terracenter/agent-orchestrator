use crate::adapters::{find_adapter_in_registry, AdaptersRegistry};
use crate::budget::{self, LoadedBudget};
use crate::capabilities::{self, TaskCapabilitiesConfig};
use crate::home_sandbox::{self, HomeCapabilitiesConfig, SandboxHome};
use crate::models::ModelsCatalog;
use crate::policy::{self, LoadedPolicy};
use crate::receipt::{now_unix, now_unix_nanos, tail_sanitized, ExecReceipt, ExecStatus};
use color_eyre::eyre::Result;
use std::collections::VecDeque;
use std::path::PathBuf;
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
    pub approval: policy::PolicyApproval,
    pub correlation_id: Option<String>,
    pub task_id: Option<String>,
    pub policy: LoadedPolicy,
    pub adapters_registry: AdaptersRegistry,
    pub task_kind: String,
    pub home_capabilities: HomeCapabilitiesConfig,
    pub task_capabilities: TaskCapabilitiesConfig,
    /// Techo diario/mensual real (issue #183). Gate ortogonal a `policy`:
    /// solo actua cuando el modelo tiene `cost_hint` en `models_catalog`.
    pub budget: LoadedBudget,
    /// Catalogo de modelos usado para resolver el `cost_hint` del modelo
    /// objetivo. `None` deshabilita el gate de presupuesto (fallback a
    /// `policy` unicamente), igual que un modelo sin `cost_hint`.
    pub models_catalog: Option<ModelsCatalog>,
    /// Ruta del state DB (SQLite) usada para leer/registrar el ledger de
    /// gasto. `None` usa `ORQ_STATE_DB` o el default de `state::open`.
    pub state_db_path: Option<String>,
    pub plan: bool,
}

#[derive(Debug)]
pub struct ApplyRequest {
    pub plan_receipt_path: String,
    pub policy: LoadedPolicy,
    pub adapters_registry: AdaptersRegistry,
    pub home_capabilities: HomeCapabilitiesConfig,
    pub task_capabilities: TaskCapabilitiesConfig,
    #[allow(dead_code)]
    pub budget: LoadedBudget,
    #[allow(dead_code)]
    pub models_catalog: Option<ModelsCatalog>,
    pub state_db_path: Option<String>,
    pub timeout_seconds: Option<u64>,
}

pub async fn run(request: ExecRequest) -> Result<ExecReceipt> {
    let started_at_unix = now_unix();
    let started = Instant::now();
    let correlation_id = request
        .correlation_id
        .clone()
        .unwrap_or_else(|| build_correlation_id(now_unix_nanos(), std::process::id()));

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

    let policy_eval = policy::evaluate(
        adapter.name(),
        &request.model,
        adapter.status(),
        &request.approval,
        &request.policy.config,
    );

    if !policy_eval.allowed {
        return Ok(ExecReceipt {
            schema_version: 1,
            correlation_id,
            agent: request.agent,
            model: request.model,
            command: Vec::new(),
            status: ExecStatus::Blocked,
            executed: false,
            policy_reason: policy_eval.reason,
            policy_source: request.policy.source.clone(),
            policy_path: request.policy.path.clone(),
            policy_sha256: request.policy.sha256.clone(),
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request.timeout_seconds,
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
            estimated_cost_usd: None,
            plan_hash: None,
        });
    }

    let model_cost_hint = request
        .models_catalog
        .as_ref()
        .and_then(|catalog| catalog.agents.get(adapter.name()))
        .and_then(|models| models.iter().find(|m| m.id == request.model))
        .and_then(|m| m.cost_hint);

    let state_db_path = request.state_db_path.as_deref().map(std::path::Path::new);
    let (spent_today_usd, spent_month_usd) = match crate::state::open(state_db_path) {
        Ok(store) => (
            store.budget_spent_today_usd().unwrap_or(0.0),
            store.budget_spent_this_month_usd().unwrap_or(0.0),
        ),
        Err(_) => (0.0, 0.0),
    };

    let budget_decision = budget::evaluate(
        model_cost_hint,
        &request.budget.config,
        spent_today_usd,
        spent_month_usd,
    );

    if !budget_decision.allowed {
        return Ok(ExecReceipt {
            schema_version: 1,
            correlation_id,
            agent: request.agent,
            model: request.model,
            command: Vec::new(),
            status: ExecStatus::Blocked,
            executed: false,
            policy_reason: format!("budget_exceeded: {}", budget_decision.reason),
            policy_source: request.policy.source.clone(),
            policy_path: request.policy.path.clone(),
            policy_sha256: request.policy.sha256.clone(),
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request.timeout_seconds,
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
            estimated_cost_usd: budget_decision.estimated_cost_usd.or(model_cost_hint),
            plan_hash: None,
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

    if request.plan {
        let estimated_cost_usd = budget_decision.estimated_cost_usd.or(model_cost_hint);
        let mut plan_receipt = ExecReceipt {
            schema_version: 1,
            correlation_id,
            agent: request.agent,
            model: request.model,
            command: command_for_receipt,
            status: ExecStatus::Planned,
            executed: false,
            policy_reason: policy_eval.reason,
            policy_source: request.policy.source,
            policy_path: request.policy.path,
            policy_sha256: request.policy.sha256,
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request.timeout_seconds,
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
            estimated_cost_usd,
            plan_hash: None,
        };
        let hash = crate::receipt::plan_receipt_sha256(&plan_receipt)?;
        plan_receipt.plan_hash = Some(hash);
        return Ok(plan_receipt);
    }

    if let Some(cost_usd) = budget_decision.estimated_cost_usd {
        if let Ok(store) = crate::state::open(state_db_path) {
            let _ = store.record_budget_spend(&crate::state::BudgetSpendInput {
                correlation_id: correlation_id.clone(),
                agent_id: request.agent.clone(),
                model_id: request.model.clone(),
                task_kind: request.task_kind.clone(),
                cost_usd,
                created_at_unix: None,
            });
        }
    }

    let Some(real_home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Ok(invalid_receipt(
            &request,
            correlation_id,
            started_at_unix,
            started,
            "HOME is not set; cannot prepare a confined sandbox HOME".to_string(),
        ));
    };
    let sandbox = match SandboxHome::prepare(
        adapter.name(),
        &request.home_capabilities,
        &real_home,
        &home_sandbox::sandbox_root(),
    ) {
        Ok(sandbox) => sandbox,
        Err(error) => {
            return Ok(invalid_receipt(
                &request,
                correlation_id,
                started_at_unix,
                started,
                format!("preparing sandbox HOME: {error}"),
            ));
        }
    };
    let task_capability = capabilities::resolve(&request.task_capabilities, &request.task_kind);
    let granted_env = capabilities::granted_env(&task_capability);

    let mut command = Command::new(&binary);
    command
        .args(&argv)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", sandbox.path())
        .envs(granted_env);
    if let Some(ref tid) = request.task_id {
        command.env("ORQ_TASK_ID", tid);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_process_group(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let cleanup_succeeded = sandbox.cleanup().await;
            return Ok(ExecReceipt {
                schema_version: 1,
                correlation_id,
                agent: request.agent,
                model: request.model,
                command: command_for_receipt,
                status: ExecStatus::SpawnFailed,
                executed: false,
                policy_reason: policy_eval.reason,
                policy_source: request.policy.source.clone(),
                policy_path: request.policy.path.clone(),
                policy_sha256: request.policy.sha256.clone(),
                started_at_unix,
                duration_ms: started.elapsed().as_millis(),
                timeout_seconds: request.timeout_seconds,
                exit_code: None,
                stdout_tail: String::new(),
                stderr_tail: format!("spawning agent {} via {}: {error}", adapter.name(), binary),
                secrets_read: false,
                cleanup_attempted: true,
                cleanup_succeeded,
                failure_class: crate::failover::classify_failure(
                    None,
                    &format!("spawning agent {} via {}: {error}", adapter.name(), binary),
                    None,
                    started.elapsed().as_millis(),
                    request.timeout_seconds,
                ),
                fallback_agent: None,
                fallback_model: None,
                fallback_reason: None,
                fallback_attempts: Vec::new(),
                estimated_cost_usd: None,
                plan_hash: None,
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

    let (status, exit_code, timeout_message, proc_cleanup_attempted, proc_cleanup_succeeded) =
        match wait_result {
            Ok(Ok(status)) => (
                if status.success() {
                    ExecStatus::Succeeded
                } else {
                    ExecStatus::Failed
                },
                status.code(),
                None,
                false,
                false,
            ),
            Ok(Err(error)) => (
                ExecStatus::SpawnFailed,
                None,
                Some(error.to_string()),
                false,
                false,
            ),
            Err(_) => {
                let (cleanup_succeeded, kill_warning) = kill_process_group(child_id);
                let _ = time::timeout(Duration::from_secs(2), child.kill()).await;
                let _ = time::timeout(Duration::from_secs(2), child.wait()).await;
                let mut message = format!("timed out after {} seconds", request.timeout_seconds);
                if let Some(warning) = kill_warning {
                    message.push_str("; ");
                    message.push_str(&warning);
                }
                (
                    ExecStatus::TimedOut,
                    None,
                    Some(message),
                    true,
                    cleanup_succeeded,
                )
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

    let sandbox_removed = sandbox.cleanup().await;
    let cleanup_attempted = true;
    let cleanup_succeeded = (!proc_cleanup_attempted || proc_cleanup_succeeded) && sandbox_removed;

    let failure_class = if status != ExecStatus::Succeeded {
        crate::failover::classify_failure(
            None,
            &stderr_tail,
            exit_code,
            started.elapsed().as_millis(),
            request.timeout_seconds,
        )
    } else {
        None
    };

    Ok(ExecReceipt {
        schema_version: 1,
        correlation_id,
        agent: request.agent,
        model: request.model,
        command: command_for_receipt,
        status,
        executed: true,
        policy_reason: policy_eval.reason,
        policy_source: request.policy.source,
        policy_path: request.policy.path,
        policy_sha256: request.policy.sha256,
        started_at_unix,
        duration_ms: started.elapsed().as_millis(),
        timeout_seconds: request.timeout_seconds,
        exit_code,
        stdout_tail,
        stderr_tail,
        secrets_read: false,
        cleanup_attempted,
        cleanup_succeeded,
        failure_class,
        fallback_agent: None,
        fallback_model: None,
        fallback_reason: None,
        fallback_attempts: Vec::new(),
        estimated_cost_usd: budget_decision.estimated_cost_usd.or(model_cost_hint),
        plan_hash: None,
    })
}

pub async fn apply(request: ApplyRequest) -> Result<ExecReceipt> {
    let started_at_unix = now_unix();
    let started = Instant::now();
    let fallback_correlation_id = format!("apply-{}", now_unix_nanos());

    let file_bytes = match tokio::fs::read(&request.plan_receipt_path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return Ok(ExecReceipt {
                schema_version: 1,
                correlation_id: fallback_correlation_id,
                agent: "unknown".to_string(),
                model: "unknown".to_string(),
                command: Vec::new(),
                status: ExecStatus::InvalidRequest,
                executed: false,
                policy_reason: format!("reading plan receipt {}: {err}", request.plan_receipt_path),
                policy_source: request.policy.source,
                policy_path: request.policy.path,
                policy_sha256: request.policy.sha256,
                started_at_unix,
                duration_ms: started.elapsed().as_millis(),
                timeout_seconds: request.timeout_seconds.unwrap_or(120),
                exit_code: None,
                stdout_tail: String::new(),
                stderr_tail: String::new(),
                secrets_read: false,
                cleanup_attempted: false,
                cleanup_succeeded: false,
                failure_class: None,
                fallback_agent: None,
                fallback_model: None,
                fallback_reason: None,
                fallback_attempts: Vec::new(),
                estimated_cost_usd: None,
                plan_hash: None,
            });
        }
    };

    let plan_receipt: ExecReceipt = match serde_json::from_slice(&file_bytes) {
        Ok(receipt) => receipt,
        Err(err) => {
            return Ok(ExecReceipt {
                schema_version: 1,
                correlation_id: fallback_correlation_id,
                agent: "unknown".to_string(),
                model: "unknown".to_string(),
                command: Vec::new(),
                status: ExecStatus::InvalidRequest,
                executed: false,
                policy_reason: format!("parsing plan receipt {}: {err}", request.plan_receipt_path),
                policy_source: request.policy.source,
                policy_path: request.policy.path,
                policy_sha256: request.policy.sha256,
                started_at_unix,
                duration_ms: started.elapsed().as_millis(),
                timeout_seconds: request.timeout_seconds.unwrap_or(120),
                exit_code: None,
                stdout_tail: String::new(),
                stderr_tail: String::new(),
                secrets_read: false,
                cleanup_attempted: false,
                cleanup_succeeded: false,
                failure_class: None,
                fallback_agent: None,
                fallback_model: None,
                fallback_reason: None,
                fallback_attempts: Vec::new(),
                estimated_cost_usd: None,
                plan_hash: None,
            });
        }
    };

    if plan_receipt.status != ExecStatus::Planned {
        return Ok(ExecReceipt {
            schema_version: plan_receipt.schema_version,
            correlation_id: plan_receipt.correlation_id,
            agent: plan_receipt.agent,
            model: plan_receipt.model,
            command: plan_receipt.command,
            status: ExecStatus::InvalidRequest,
            executed: false,
            policy_reason: format!(
                "cannot apply receipt with status '{:?}'; expected 'planned'",
                plan_receipt.status
            ),
            policy_source: request.policy.source,
            policy_path: request.policy.path,
            policy_sha256: request.policy.sha256,
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request
                .timeout_seconds
                .unwrap_or(plan_receipt.timeout_seconds),
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
            estimated_cost_usd: plan_receipt.estimated_cost_usd,
            plan_hash: plan_receipt.plan_hash,
        });
    }

    let Some(ref expected_hash) = plan_receipt.plan_hash else {
        return Ok(ExecReceipt {
            schema_version: plan_receipt.schema_version,
            correlation_id: plan_receipt.correlation_id,
            agent: plan_receipt.agent,
            model: plan_receipt.model,
            command: plan_receipt.command,
            status: ExecStatus::InvalidRequest,
            executed: false,
            policy_reason: "plan receipt is missing plan_hash".to_string(),
            policy_source: request.policy.source,
            policy_path: request.policy.path,
            policy_sha256: request.policy.sha256,
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request
                .timeout_seconds
                .unwrap_or(plan_receipt.timeout_seconds),
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
            estimated_cost_usd: plan_receipt.estimated_cost_usd,
            plan_hash: None,
        });
    };

    let actual_hash = match crate::receipt::plan_receipt_sha256(&plan_receipt) {
        Ok(hash) => hash,
        Err(err) => {
            return Ok(ExecReceipt {
                schema_version: plan_receipt.schema_version,
                correlation_id: plan_receipt.correlation_id,
                agent: plan_receipt.agent,
                model: plan_receipt.model,
                command: plan_receipt.command,
                status: ExecStatus::InvalidRequest,
                executed: false,
                policy_reason: format!("calculating plan hash: {err}"),
                policy_source: request.policy.source,
                policy_path: request.policy.path,
                policy_sha256: request.policy.sha256,
                started_at_unix,
                duration_ms: started.elapsed().as_millis(),
                timeout_seconds: request
                    .timeout_seconds
                    .unwrap_or(plan_receipt.timeout_seconds),
                exit_code: None,
                stdout_tail: String::new(),
                stderr_tail: String::new(),
                secrets_read: false,
                cleanup_attempted: false,
                cleanup_succeeded: false,
                failure_class: None,
                fallback_agent: None,
                fallback_model: None,
                fallback_reason: None,
                fallback_attempts: Vec::new(),
                estimated_cost_usd: plan_receipt.estimated_cost_usd,
                plan_hash: plan_receipt.plan_hash,
            });
        }
    };

    if &actual_hash != expected_hash {
        return Ok(ExecReceipt {
            schema_version: plan_receipt.schema_version,
            correlation_id: plan_receipt.correlation_id,
            agent: plan_receipt.agent,
            model: plan_receipt.model,
            command: plan_receipt.command,
            status: ExecStatus::InvalidRequest,
            executed: false,
            policy_reason: format!(
                "plan receipt hash verification failed: expected {}, got {}",
                expected_hash, actual_hash
            ),
            policy_source: request.policy.source,
            policy_path: request.policy.path,
            policy_sha256: request.policy.sha256,
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request
                .timeout_seconds
                .unwrap_or(plan_receipt.timeout_seconds),
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
            estimated_cost_usd: plan_receipt.estimated_cost_usd,
            plan_hash: plan_receipt.plan_hash,
        });
    }

    let Some(adapter) = find_adapter_in_registry(&plan_receipt.agent, &request.adapters_registry)
    else {
        return Ok(ExecReceipt {
            schema_version: plan_receipt.schema_version,
            correlation_id: plan_receipt.correlation_id,
            agent: plan_receipt.agent.clone(),
            model: plan_receipt.model.clone(),
            command: plan_receipt.command,
            status: ExecStatus::InvalidRequest,
            executed: false,
            policy_reason: format!("unknown agent adapter: {}", plan_receipt.agent),
            policy_source: request.policy.source,
            policy_path: request.policy.path,
            policy_sha256: request.policy.sha256,
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds: request
                .timeout_seconds
                .unwrap_or(plan_receipt.timeout_seconds),
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
            estimated_cost_usd: plan_receipt.estimated_cost_usd,
            plan_hash: plan_receipt.plan_hash,
        });
    };

    let state_db_path = request.state_db_path.as_deref().map(std::path::Path::new);
    if let Some(cost_usd) = plan_receipt.estimated_cost_usd {
        if let Ok(store) = crate::state::open(state_db_path) {
            let _ = store.record_budget_spend(&crate::state::BudgetSpendInput {
                correlation_id: plan_receipt.correlation_id.clone(),
                agent_id: plan_receipt.agent.clone(),
                model_id: plan_receipt.model.clone(),
                task_kind: "apply".to_string(),
                cost_usd,
                created_at_unix: None,
            });
        }
    }

    let timeout_seconds = request
        .timeout_seconds
        .unwrap_or(plan_receipt.timeout_seconds)
        .clamp(1, MAX_TIMEOUT_SECONDS);
    let binary = adapter
        .binary_path()
        .unwrap_or_else(|| adapter.binary().to_string());
    let argv = adapter.build_argv(&plan_receipt.model, "");

    let Some(real_home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Ok(ExecReceipt {
            schema_version: plan_receipt.schema_version,
            correlation_id: plan_receipt.correlation_id,
            agent: plan_receipt.agent,
            model: plan_receipt.model,
            command: plan_receipt.command,
            status: ExecStatus::InvalidRequest,
            executed: false,
            policy_reason: "HOME is not set; cannot prepare a confined sandbox HOME".to_string(),
            policy_source: request.policy.source,
            policy_path: request.policy.path,
            policy_sha256: request.policy.sha256,
            started_at_unix,
            duration_ms: started.elapsed().as_millis(),
            timeout_seconds,
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
            estimated_cost_usd: plan_receipt.estimated_cost_usd,
            plan_hash: plan_receipt.plan_hash,
        });
    };

    let sandbox = match SandboxHome::prepare(
        adapter.name(),
        &request.home_capabilities,
        &real_home,
        &home_sandbox::sandbox_root(),
    ) {
        Ok(sandbox) => sandbox,
        Err(error) => {
            return Ok(ExecReceipt {
                schema_version: plan_receipt.schema_version,
                correlation_id: plan_receipt.correlation_id,
                agent: plan_receipt.agent,
                model: plan_receipt.model,
                command: plan_receipt.command,
                status: ExecStatus::InvalidRequest,
                executed: false,
                policy_reason: format!("preparing sandbox HOME: {error}"),
                policy_source: request.policy.source,
                policy_path: request.policy.path,
                policy_sha256: request.policy.sha256,
                started_at_unix,
                duration_ms: started.elapsed().as_millis(),
                timeout_seconds,
                exit_code: None,
                stdout_tail: String::new(),
                stderr_tail: String::new(),
                secrets_read: false,
                cleanup_attempted: false,
                cleanup_succeeded: false,
                failure_class: None,
                fallback_agent: None,
                fallback_model: None,
                fallback_reason: None,
                fallback_attempts: Vec::new(),
                estimated_cost_usd: plan_receipt.estimated_cost_usd,
                plan_hash: plan_receipt.plan_hash,
            });
        }
    };

    let task_capability = capabilities::resolve(&request.task_capabilities, "unspecified");
    let granted_env = capabilities::granted_env(&task_capability);

    let mut command = Command::new(&binary);
    command
        .args(&argv)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", sandbox.path())
        .envs(granted_env);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_process_group(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let cleanup_succeeded = sandbox.cleanup().await;
            return Ok(ExecReceipt {
                schema_version: plan_receipt.schema_version,
                correlation_id: plan_receipt.correlation_id,
                agent: plan_receipt.agent,
                model: plan_receipt.model,
                command: plan_receipt.command,
                status: ExecStatus::SpawnFailed,
                executed: false,
                policy_reason: plan_receipt.policy_reason,
                policy_source: request.policy.source,
                policy_path: request.policy.path,
                policy_sha256: request.policy.sha256,
                started_at_unix,
                duration_ms: started.elapsed().as_millis(),
                timeout_seconds,
                exit_code: None,
                stdout_tail: String::new(),
                stderr_tail: format!("spawning agent {} via {}: {error}", adapter.name(), binary),
                secrets_read: false,
                cleanup_attempted: true,
                cleanup_succeeded,
                failure_class: crate::failover::classify_failure(
                    None,
                    &format!("spawning agent {} via {}: {error}", adapter.name(), binary),
                    None,
                    started.elapsed().as_millis(),
                    timeout_seconds,
                ),
                fallback_agent: None,
                fallback_model: None,
                fallback_reason: None,
                fallback_attempts: Vec::new(),
                estimated_cost_usd: plan_receipt.estimated_cost_usd,
                plan_hash: plan_receipt.plan_hash,
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

    let wait_result = time::timeout(Duration::from_secs(timeout_seconds), child.wait()).await;

    let (status, exit_code, timeout_message, proc_cleanup_attempted, proc_cleanup_succeeded) =
        match wait_result {
            Ok(Ok(status)) => (
                if status.success() {
                    ExecStatus::Succeeded
                } else {
                    ExecStatus::Failed
                },
                status.code(),
                None,
                false,
                false,
            ),
            Ok(Err(error)) => (
                ExecStatus::SpawnFailed,
                None,
                Some(error.to_string()),
                false,
                false,
            ),
            Err(_) => {
                let (cleanup_succeeded, kill_warning) = kill_process_group(child_id);
                let _ = time::timeout(Duration::from_secs(2), child.kill()).await;
                let _ = time::timeout(Duration::from_secs(2), child.wait()).await;
                let mut message = format!("timed out after {} seconds", timeout_seconds);
                if let Some(warning) = kill_warning {
                    message.push_str("; ");
                    message.push_str(&warning);
                }
                (
                    ExecStatus::TimedOut,
                    None,
                    Some(message),
                    true,
                    cleanup_succeeded,
                )
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

    let sandbox_removed = sandbox.cleanup().await;
    let cleanup_attempted = true;
    let cleanup_succeeded = (!proc_cleanup_attempted || proc_cleanup_succeeded) && sandbox_removed;

    let failure_class = if status != ExecStatus::Succeeded {
        crate::failover::classify_failure(
            None,
            &stderr_tail,
            exit_code,
            started.elapsed().as_millis(),
            timeout_seconds,
        )
    } else {
        None
    };

    Ok(ExecReceipt {
        schema_version: 1,
        correlation_id: plan_receipt.correlation_id,
        agent: plan_receipt.agent,
        model: plan_receipt.model,
        command: plan_receipt.command,
        status,
        executed: true,
        policy_reason: plan_receipt.policy_reason,
        policy_source: request.policy.source,
        policy_path: request.policy.path,
        policy_sha256: request.policy.sha256,
        started_at_unix,
        duration_ms: started.elapsed().as_millis(),
        timeout_seconds,
        exit_code,
        stdout_tail,
        stderr_tail,
        secrets_read: false,
        cleanup_attempted,
        cleanup_succeeded,
        failure_class,
        fallback_agent: None,
        fallback_model: None,
        fallback_reason: None,
        fallback_attempts: Vec::new(),
        estimated_cost_usd: plan_receipt.estimated_cost_usd,
        plan_hash: plan_receipt.plan_hash,
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
fn kill_process_group(child_id: Option<u32>) -> (bool, Option<String>) {
    let Some(pid) = child_id else {
        return (
            false,
            Some("child pid missing for process group kill".to_string()),
        );
    };
    let result = unsafe { libc::kill(-(pid as libc::pid_t), libc::SIGKILL) };
    if result == 0 {
        (true, None)
    } else {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            (true, None)
        } else {
            (
                false,
                Some(format!("kill process group {} failed: {}", pid, err)),
            )
        }
    }
}

#[cfg(not(unix))]
fn kill_process_group(_child_id: Option<u32>) -> (bool, Option<String>) {
    (
        false,
        Some("process group kill is unsupported on this platform".to_string()),
    )
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
        executed: false,
        policy_reason: reason,
        policy_source: request.policy.source.clone(),
        policy_path: request.policy.path.clone(),
        policy_sha256: request.policy.sha256.clone(),
        started_at_unix,
        duration_ms: started.elapsed().as_millis(),
        timeout_seconds: request.timeout_seconds,
        exit_code: None,
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        secrets_read: false,
        cleanup_attempted: false,
        cleanup_succeeded: false,
        failure_class: None,
        fallback_agent: None,
        fallback_model: None,
        fallback_reason: None,
        fallback_attempts: Vec::new(),
        estimated_cost_usd: None,
        plan_hash: None,
    }
}

fn build_correlation_id(nanos: u128, pid: u32) -> String {
    format!("orq-agent-{nanos}-{pid}")
}

#[cfg(test)]
mod tests {
    use super::build_correlation_id;

    #[test]
    fn correlation_id_unique_across_pids() {
        assert_eq!(
            build_correlation_id(1788535346, 10),
            "orq-agent-1788535346-10"
        );
        assert_ne!(
            build_correlation_id(1788535346, 10),
            build_correlation_id(1788535346, 11)
        );
    }
}
