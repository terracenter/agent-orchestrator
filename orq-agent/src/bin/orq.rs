#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
    orq_agent::run_cli().await
}
