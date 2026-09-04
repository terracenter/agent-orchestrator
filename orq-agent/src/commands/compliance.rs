use crate::compliance::{run_compliance, ComplianceArgs, ComplianceReport};
use color_eyre::eyre::Result;

pub async fn run(args: ComplianceArgs) -> Result<ComplianceReport> {
    run_compliance(args).await
}
