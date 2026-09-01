use clap::{Parser, Subcommand, ValueEnum};
use color_eyre::eyre::Result;
use serde::Serialize;

mod adapters;
mod detect;
mod exec;
mod models;
mod policy;
mod receipt;
mod route;
mod smoke;

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
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Report known/candidate models for an agent without reading secrets.
    Models {
        /// Agent adapter name.
        #[arg(long)]
        agent: String,
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
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
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
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Json,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    match cli.command {
        Commands::Detect { format } => print_json(format, &detect::detect_agents()),
        Commands::Exec {
            agent,
            model,
            task_file,
            timeout,
            allow_gated,
            correlation_id,
            policy_config,
            format,
        } => {
            let policy_config_path = policy_config.as_deref().map(std::path::Path::new);
            let (policy_config, _) = policy::load_config(policy_config_path).await?;
            let receipt = exec::run(exec::ExecRequest {
                agent,
                model,
                task_file,
                timeout_seconds: timeout,
                allow_gated,
                correlation_id,
                policy_config,
            })
            .await?;
            print_json(format, &receipt)
        }
        Commands::Models { agent, format } => print_json(format, &models::list(&agent)?),
        Commands::Route {
            task_kind,
            config,
            allow_gated,
            format,
        } => {
            let config_path = config.as_deref().map(std::path::Path::new);
            let (routing_config, config_source) = route::load_config(config_path).await?;
            let decision = route::decide(&routing_config, &task_kind, allow_gated, &config_source)?;
            print_json(format, &decision)
        }
        Commands::Smoke {
            agent,
            model,
            timeout,
            allow_gated,
            correlation_id,
            policy_config,
            format,
        } => {
            let policy_config_path = policy_config.as_deref().map(std::path::Path::new);
            let (policy_config, _) = policy::load_config(policy_config_path).await?;
            let receipt = smoke::run(
                agent,
                model,
                timeout,
                allow_gated,
                correlation_id,
                policy_config,
            )
            .await?;
            print_json(format, &receipt)
        }
    }
}

fn print_json<T: Serialize>(format: OutputFormat, value: &T) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
    }
    Ok(())
}
