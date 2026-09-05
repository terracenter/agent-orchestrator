use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use serde::Serialize;

mod adapters;
mod certify;
mod certstore;
pub mod compliance;
mod commands;
mod detect;
mod discover;
mod exec;
mod models;
mod policy;
mod quota;
mod receipt;
mod route;
mod smoke;
mod state;
pub mod delegate;

#[derive(Debug, Parser)]
#[command(about = "Real local dispatcher for Orq agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Detect local agent runners without reading secrets.
    Detect {
        /// Optional adapters registry JSON path. Uses bundled config when omitted.
        #[arg(long)]
        adapters_config: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Discover local agents/models and persist the catalog into SQLite state.
    Discover {
        /// Optional adapters registry JSON path. Uses bundled config when omitted.
        #[arg(long)]
        adapters_config: Option<String>,
        /// Optional models catalog JSON path. Uses bundled config when omitted.
        #[arg(long)]
        models_config: Option<String>,
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Execute a real local agent runner and emit a verifiable JSON receipt.
    Exec {
        /// Agent adapter name.
        #[arg(long)]
        agent: String,
        /// Model identifier passed to the adapter.
        #[arg(long)]
        model: String,
        /// Markdown/text task file to send as prompt.
        #[arg(long)]
        task_file: String,
        /// Timeout in seconds.
        #[arg(long, default_value_t = 120)]
        timeout: u64,
        /// Allow gated agents/models after explicit human approval.
        #[arg(long, default_value_t = false)]
        allow_gated: bool,
        /// Correlation id propagated from Orq legacy/Observer.
        #[arg(long)]
        correlation_id: Option<String>,
        /// Optional policy config JSON path. Uses bundled config when omitted.
        #[arg(long)]
        policy_config: Option<String>,
        /// Optional adapters registry JSON path. Uses bundled config when omitted.
        #[arg(long)]
        adapters_config: Option<String>,
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Report known/candidate models for an agent without reading secrets.
    Models {
        /// Agent adapter name.
        #[arg(long)]
        agent: Option<String>,
        /// Optional models catalog JSON path. Uses bundled config when omitted.
        #[arg(long)]
        config: Option<String>,
        /// Optional adapters registry JSON path. Uses bundled config when omitted.
        #[arg(long)]
        adapters_config: Option<String>,
        /// Subcommand for models management (e.g. refresh).
        #[command(subcommand)]
        command: Option<ModelsSubcommand>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Recommend an agent/model for a task kind using a routing matrix config.
    Route {
        /// Task kind defined by the routing config.
        #[arg(long)]
        task_kind: String,
        /// Optional routing config JSON path. Uses bundled config when omitted.
        #[arg(long)]
        config: Option<String>,
        /// Allow gated agents/models after explicit human approval.
        #[arg(long, default_value_t = false)]
        allow_gated: bool,
        /// Optional adapters registry JSON path. Uses bundled config when omitted.
        #[arg(long)]
        adapters_config: Option<String>,
        /// Optional models catalog JSON path. Uses bundled config when omitted.
        #[arg(long)]
        models_config: Option<String>,
        /// Optional certificate directory for routing preferences.
        #[arg(long)]
        cert_dir: Option<String>,
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Certify an agent/model/task-kind with a bounded smoke receipt.
    Certify {
        /// Agent adapter name.
        #[arg(long)]
        agent: String,
        /// Model identifier passed to the adapter.
        #[arg(long)]
        model: String,
        /// Task kind being certified.
        #[arg(long)]
        task_kind: String,
        /// Timeout in seconds.
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        /// Allow gated agents/models after explicit human approval.
        #[arg(long, default_value_t = false)]
        allow_gated: bool,
        /// Correlation id propagated from Orq legacy/Observer.
        #[arg(long)]
        correlation_id: Option<String>,
        /// Optional certificate output path.
        #[arg(long)]
        output: Option<String>,
        /// Optional policy config JSON path. Uses bundled config when omitted.
        #[arg(long)]
        policy_config: Option<String>,
        /// Optional adapters registry JSON path. Uses bundled config when omitted.
        #[arg(long)]
        adapters_config: Option<String>,
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Manage the local SQLite state store.
    State {
        #[command(subcommand)]
        command: StateCommand,
    },
    /// Manage and report agent provider quotas.
    Quota {
        #[command(subcommand)]
        command: QuotaCommand,
    },
    /// Execute a bounded real smoke task and emit a receipt.
    Smoke {
        /// Agent adapter name.
        #[arg(long)]
        agent: String,
        /// Model identifier passed to the adapter.
        #[arg(long)]
        model: String,
        /// Timeout in seconds.
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        /// Allow gated agents/models after explicit human approval.
        #[arg(long, default_value_t = false)]
        allow_gated: bool,
        /// Correlation id propagated from Orq legacy/Observer.
        #[arg(long)]
        correlation_id: Option<String>,
        /// Optional policy config JSON path. Uses bundled config when omitted.
        #[arg(long)]
        policy_config: Option<String>,
        /// Optional adapters registry JSON path. Uses bundled config when omitted.
        #[arg(long)]
        adapters_config: Option<String>,
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Audit agent compliance for workspace rules (rtk, engram, vg).
    Compliance {
        /// Optional path to log file or directory to scan for raw command usage.
        #[arg(long)]
        log: Option<String>,
        /// Project name for engram session_summary verification.
        #[arg(long)]
        project: Option<String>,
        /// Optional vault directory path for vg-sync check. Resolved only from --vault-path or ORQ_VAULT_PATH / VAULT_PATH env vars.
        #[arg(long)]
        vault_path: Option<String>,
        /// Optional kuzu db path or sync marker path. Resolved only from --kuzu-path or ORQ_KUZU_PATH / KUZU_PATH env vars.
        #[arg(long)]
        kuzu_path: Option<String>,
        /// Optional custom engram binary path (for testing or overrides). Defaults to ORQ_ENGRAM_BIN or engram.
        #[arg(long)]
        engram_bin: Option<String>,
        /// Enable only rtk-usage check.
        #[arg(long, default_value_t = false)]
        rtk_usage: bool,
        /// Enable only engram-summary check.
        #[arg(long, default_value_t = false)]
        engram_summary: bool,
        /// Enable only vg-sync check.
        #[arg(long, default_value_t = false)]
        vg_sync: bool,
        /// Optional agent name being audited.
        #[arg(long)]
        agent: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Delegate execution to an external or autonomous agent and record verifiable receipt.
    Delegate {
        /// Task prompt or description to delegate.
        #[arg(long)]
        task: Option<String>,
        /// Target agent runner (e.g. agy, hermes, openclaw, pi).
        #[arg(long)]
        agent: Option<String>,
        /// Target model identifier.
        #[arg(long)]
        model: Option<String>,
        /// Source handoff markdown path.
        #[arg(long)]
        handoff: Option<String>,
        /// Repository directory path.
        #[arg(long)]
        repo_path: Option<String>,
        /// Agents workspace directory path.
        #[arg(long)]
        agents_dir: Option<String>,
        /// Workspace root path.
        #[arg(long)]
        workspace: Option<String>,
        /// Path to write generated handoff markdown file.
        #[arg(long)]
        write_handoff: Option<String>,
        /// Path to write generated receipt JSON file.
        #[arg(long)]
        write_receipt: Option<String>,
        /// Force overwrite existing handoff or receipt files.
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Execute the agent runner directly instead of emitting plan/command only.
        #[arg(long, default_value_t = false)]
        execute: bool,
        /// Timeout in seconds for direct execution.
        #[arg(long, default_value_t = 120)]
        timeout: u64,
        /// Correlation id propagated from caller.
        #[arg(long)]
        correlation_id: Option<String>,
        /// Optional policy config JSON path. Uses bundled config when omitted.
        #[arg(long)]
        policy_config: Option<String>,
        /// Optional adapters registry JSON path. Uses bundled config when omitted.
        #[arg(long)]
        adapters_config: Option<String>,
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum OutputFormat {
    Json,
    Text,
}

#[derive(Debug, Subcommand)]
enum ModelsSubcommand {
    /// Refresh the models catalog from a market feed.
    Refresh {
        /// Optional market feed JSON path. Uses bundled config or env when omitted.
        #[arg(long)]
        feed: Option<String>,
        /// Optional models catalog JSON path. Uses bundled config or env when omitted.
        #[arg(long)]
        catalog: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum QuotaCommand {
    /// Ingest or record a quota snapshot manually or from JSON.
    Record {
        /// Provider identifier (e.g., agy, claude-code, codex, qwen).
        #[arg(long)]
        provider: Option<String>,
        /// Quota scope/group (e.g., weekly, five_hour, session, daily).
        #[arg(long)]
        scope: Option<String>,
        /// Remaining quota percentage (0.0 - 100.0).
        #[arg(long)]
        remaining_pct: Option<f64>,
        /// Used quota percentage (0.0 - 100.0).
        #[arg(long)]
        used_pct: Option<f64>,
        /// Quota status (ok, quota_unknown, exceeded, warning, exhausted).
        #[arg(long)]
        status: Option<String>,
        /// Unix timestamp when the quota resets or refreshes.
        #[arg(long)]
        reset_at_unix: Option<u64>,
        /// Relative seconds until quota reset/refresh.
        #[arg(long)]
        reset_in_seconds: Option<u64>,
        /// Captured timestamp in unix seconds. Defaults to now.
        #[arg(long)]
        captured_at_unix: Option<u64>,
        /// Optional JSON metadata string or payload.
        #[arg(long)]
        metadata: Option<String>,
        /// Raw JSON string or @file.json containing single snapshot or array of snapshots.
        #[arg(long)]
        json: Option<String>,
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Report latest quota snapshots per provider and scope.
    Report {
        /// Optional provider filter.
        #[arg(long)]
        provider: Option<String>,
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum StateCommand {
    /// Apply SQLite state migrations.
    Migrate {
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Report SQLite state status.
    Status {
        /// Optional state DB path. Uses ORQ_STATE_DB or default when omitted.
        #[arg(long)]
        db_path: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

pub async fn run_cli() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    run_command(cli.command).await
}

async fn run_command(command: Commands) -> Result<()> {
    match command {
        Commands::Detect {
            adapters_config,
            format,
        } => {
            let report =
                commands::detect::run(commands::detect::DetectArgs { adapters_config }).await?;
            print_json(format, &report)
        }
        Commands::Discover {
            adapters_config,
            models_config,
            db_path,
            format,
        } => {
            let report = discover::run(discover::DiscoverRequest {
                adapters_config: adapters_config.as_deref().map(std::path::Path::new),
                models_config: models_config.as_deref().map(std::path::Path::new),
                state_db_path: db_path.as_deref().map(std::path::Path::new),
            })
            .await?;
            print_json(format, &report)
        }
        Commands::Exec {
            agent,
            model,
            task_file,
            timeout,
            allow_gated,
            correlation_id,
            policy_config,
            adapters_config,
            db_path,
            format,
        } => {
            let policy_config_path = policy_config.as_deref().map(std::path::Path::new);
            let (policy_config, _) = policy::load_config(policy_config_path).await?;
            let adapters_config_path = adapters_config.as_deref().map(std::path::Path::new);
            let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;
            let receipt = exec::run(exec::ExecRequest {
                agent,
                model,
                task_file,
                timeout_seconds: timeout,
                allow_gated,
                correlation_id,
                policy_config,
                adapters_registry,
            })
            .await?;
            persist_exec_receipt(db_path.as_deref(), &receipt, "exec");
            record_exec_breaker_outcome(db_path.as_deref(), &receipt);
            print_json(format, &receipt)
        }
        Commands::Models {
            agent,
            config,
            adapters_config,
            command,
            format,
        } => match command {
            Some(ModelsSubcommand::Refresh {
                feed,
                catalog,
                format: refresh_format,
            }) => {
                let summary = commands::models::run_refresh(commands::models::ModelsRefreshArgs {
                    feed,
                    catalog,
                })
                .await?;
                print_json(refresh_format, &summary)
            }
            None => {
                let agent = agent.ok_or_else(|| {
                    color_eyre::eyre::eyre!("missing required argument '--agent <AGENT>'")
                })?;
                let report = commands::models::run(commands::models::ModelsArgs {
                    agent,
                    config,
                    adapters_config,
                })
                .await?;
                print_json(format, &report)
            }
        },
        Commands::Route {
            task_kind,
            config,
            allow_gated,
            adapters_config,
            models_config,
            cert_dir,
            db_path,
            format,
        } => {
            let decision = commands::route::run(commands::route::RouteArgs {
                task_kind,
                config,
                allow_gated,
                adapters_config,
                models_config,
                cert_dir,
                db_path,
            })
            .await?;
            print_json(format, &decision)
        }
        Commands::Certify {
            agent,
            model,
            task_kind,
            timeout,
            allow_gated,
            correlation_id,
            output,
            policy_config,
            adapters_config,
            db_path,
            format,
        } => {
            let policy_config_path = policy_config.as_deref().map(std::path::Path::new);
            let (policy_config, _) = policy::load_config(policy_config_path).await?;
            let adapters_config_path = adapters_config.as_deref().map(std::path::Path::new);
            let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;
            let certificate = certify::run(certify::CertifyRequest {
                agent,
                model,
                task_kind,
                timeout_seconds: timeout,
                allow_gated,
                correlation_id,
                output,
                policy_config,
                adapters_registry,
            })
            .await?;
            persist_exec_receipt(
                db_path.as_deref(),
                &certificate.receipt,
                &certificate.task_kind,
            );
            record_exec_breaker_outcome(db_path.as_deref(), &certificate.receipt);
            print_json(format, &certificate)
        }
        Commands::Quota { command } => match command {
            QuotaCommand::Record {
                provider,
                scope,
                remaining_pct,
                used_pct,
                status,
                reset_at_unix,
                reset_in_seconds,
                captured_at_unix,
                metadata,
                json,
                db_path,
                format,
            } => {
                let response = commands::quota::run_record(commands::quota::QuotaRecordArgs {
                    provider,
                    scope,
                    remaining_pct,
                    used_pct,
                    status,
                    reset_at_unix,
                    reset_in_seconds,
                    captured_at_unix,
                    metadata,
                    json,
                    db_path,
                })
                .await?;
                print_json(format, &response)
            }
            QuotaCommand::Report {
                provider,
                db_path,
                format,
            } => {
                let report = commands::quota::run_report(commands::quota::QuotaReportArgs {
                    provider,
                    db_path,
                })
                .await?;
                print_json(format, &report)
            }
        },
        Commands::State { command } => match command {
            StateCommand::Migrate { db_path, format }
            | StateCommand::Status { db_path, format } => {
                let path = db_path.as_deref().map(std::path::Path::new);
                let store = state::open(path)?;
                let status = store.status()?;
                print_json(format, &status)
            }
        },
        Commands::Smoke {
            agent,
            model,
            timeout,
            allow_gated,
            correlation_id,
            policy_config,
            adapters_config,
            db_path,
            format,
        } => {
            let policy_config_path = policy_config.as_deref().map(std::path::Path::new);
            let (policy_config, _) = policy::load_config(policy_config_path).await?;
            let adapters_config_path = adapters_config.as_deref().map(std::path::Path::new);
            let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;
            let receipt = smoke::run(
                agent,
                model,
                timeout,
                allow_gated,
                correlation_id,
                policy_config,
                adapters_registry,
            )
            .await?;
            persist_exec_receipt(db_path.as_deref(), &receipt, "smoke");
            record_exec_breaker_outcome(db_path.as_deref(), &receipt);
            print_json(format, &receipt)
        }
        Commands::Compliance {
            log,
            project,
            vault_path,
            kuzu_path,
            engram_bin,
            rtk_usage,
            engram_summary,
            vg_sync,
            agent,
            format,
        } => {
            let report = commands::compliance::run(compliance::ComplianceArgs {
                log,
                project,
                vault_path,
                kuzu_path,
                engram_bin,
                rtk_usage,
                engram_summary,
                vg_sync,
                agent,
            })
            .await?;

            print_compliance_report(format, &report)?;
            if report.exit_code != 0 {
                std::process::exit(report.exit_code);
            }
            Ok(())
        }
        Commands::Delegate {
            task,
            agent,
            model,
            handoff,
            repo_path,
            agents_dir,
            workspace,
            write_handoff,
            write_receipt,
            force,
            execute,
            timeout,
            correlation_id,
            policy_config,
            adapters_config,
            db_path,
            format,
        } => {
            let output = commands::delegate::run(commands::delegate::DelegateArgs {
                task,
                agent,
                model,
                handoff,
                repo_path,
                agents_dir,
                workspace,
                write_handoff,
                write_receipt,
                force,
                execute,
                timeout_seconds: timeout,
                correlation_id,
                policy_config,
                adapters_config,
            })
            .await?;
            persist_delegate_receipt(db_path.as_deref(), &output.receipt, "delegate");
            print_json(format, &output)
        }
    }
}

fn print_compliance_report(
    format: OutputFormat,
    report: &compliance::ComplianceReport,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(report)?);
        }
        OutputFormat::Text => {
            println!("=== Agent Compliance Report ===");
            if let Some(ref agent) = report.agent {
                println!("Agent:       {}", agent);
            }
            println!("Status:      {:?}", report.status);
            println!("Exit Code:   {}", report.exit_code);
            println!("Summary:     {}", report.summary);
            println!("--------------------------------");
            if let Some(ref r) = report.checks.rtk_usage {
                println!(
                    "[rtk-usage]      Status: {:?} | Raw Invocations: {} | {}",
                    r.status, r.raw_invocations_count, r.message
                );
                for v in &r.violations {
                    println!(
                        "  - {}:{} | Binary: {} | Command: {}",
                        v.file, v.line, v.binary, v.raw_command
                    );
                }
            }
            if let Some(ref e) = report.checks.engram_summary {
                println!(
                    "[engram-summary] Status: {:?} | Target Date: {} | Project: {} | Summaries: {} | {}",
                    e.status, e.target_date, e.project, e.session_summaries_count, e.message
                );
            }
            if let Some(ref v) = report.checks.vg_sync {
                println!(
                    "[vg-sync]        Status: {:?} | Fresh: {} | {}",
                    v.status, v.is_fresh, v.message
                );
            }
        }
    }
    Ok(())
}

fn persist_exec_receipt(db_path: Option<&str>, receipt: &receipt::ExecReceipt, task_kind: &str) {
    let path = db_path.map(std::path::Path::new);
    let Ok(store) = state::open(path) else {
        return;
    };
    if let Err(err) = store.insert_receipt(receipt, task_kind) {
        eprintln!("warning: failed to persist execution receipt: {err}");
    }
}

fn persist_delegate_receipt(
    db_path: Option<&str>,
    receipt: &receipt::DelegateReceipt,
    task_kind: &str,
) {
    let path = db_path.map(std::path::Path::new);
    let Ok(store) = state::open(path) else {
        return;
    };
    if let Err(err) = store.insert_delegate_receipt(receipt, task_kind) {
        eprintln!("warning: failed to persist delegate receipt: {err}");
    }
}

fn record_exec_breaker_outcome(db_path: Option<&str>, receipt: &receipt::ExecReceipt) {
    let Some(outcome) = map_receipt_to_breaker_outcome(receipt) else {
        return;
    };
    let path = db_path.map(std::path::Path::new);
    let Ok(store) = state::open(path) else {
        return;
    };
    if let Err(err) = store.record_breaker_outcome(&receipt.agent, &receipt.model, outcome) {
        eprintln!("warning: failed to record circuit breaker outcome: {err}");
    }
}

fn map_receipt_to_breaker_outcome(receipt: &receipt::ExecReceipt) -> Option<state::BreakerOutcome> {
    match receipt.status {
        receipt::ExecStatus::Succeeded => Some(state::BreakerOutcome::Success),
        receipt::ExecStatus::TimedOut => Some(state::BreakerOutcome::TimedOut),
        receipt::ExecStatus::Failed | receipt::ExecStatus::SpawnFailed => {
            let is_auth =
                is_auth_failure(&receipt.stderr_tail) || is_auth_failure(&receipt.stdout_tail);
            if is_auth {
                Some(state::BreakerOutcome::Auth401)
            } else {
                Some(state::BreakerOutcome::AdapterError)
            }
        }
        receipt::ExecStatus::Blocked | receipt::ExecStatus::InvalidRequest => None,
    }
}

fn is_auth_failure(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("401")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
        || lower.contains("auth error")
        || lower.contains("auth failed")
        || lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
        || lower.contains("permission_denied")
}

fn print_json<T: Serialize>(format: OutputFormat, value: &T) -> Result<()> {
    match format {
        OutputFormat::Json | OutputFormat::Text => {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
    }
    Ok(())
}
