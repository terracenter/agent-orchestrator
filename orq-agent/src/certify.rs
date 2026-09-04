use crate::adapters::AdaptersRegistry;
use crate::policy::PolicyConfig;
use crate::receipt::{now_unix, receipt_sha256, ExecReceipt, ExecStatus};
use crate::smoke;
use color_eyre::eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug)]
pub struct CertifyRequest {
    pub agent: String,
    pub model: String,
    pub task_kind: String,
    pub timeout_seconds: u64,
    pub allow_gated: bool,
    pub correlation_id: Option<String>,
    pub output: Option<String>,
    pub policy_config: PolicyConfig,
    pub adapters_registry: AdaptersRegistry,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Certificate {
    pub schema_version: u8,
    pub certificate_id: String,
    pub created_at_unix: u64,
    pub agent: String,
    pub model: String,
    pub task_kind: String,
    pub status: CertificateStatus,
    pub receipt_sha256: String,
    pub receipt: ExecReceipt,
    pub output_path: Option<String>,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CertificateStatus {
    Certified,
    Failed,
}

pub async fn run(request: CertifyRequest) -> Result<Certificate> {
    let receipt = smoke::run(
        request.agent.clone(),
        request.model.clone(),
        request.timeout_seconds,
        request.allow_gated,
        request.correlation_id,
        request.policy_config,
        request.adapters_registry,
    )
    .await?;
    let receipt_sha256 = receipt_sha256(&receipt)?;
    let created_at_unix = now_unix();
    let certificate_id = format!(
        "cert-{created_at_unix}-{}-{}-{}",
        safe_id(&request.agent),
        safe_id(&request.model),
        safe_id(&request.task_kind)
    );
    let status = certificate_status_for_receipt(&receipt);
    let output_path = request.output.clone();
    let secrets_read = receipt.secrets_read;
    let certificate = Certificate {
        schema_version: 1,
        certificate_id,
        created_at_unix,
        agent: request.agent,
        model: request.model,
        task_kind: request.task_kind,
        status,
        receipt_sha256,
        receipt,
        output_path,
        secrets_read,
    };

    if let Some(path) = &request.output {
        write_certificate(Path::new(path), &certificate).await?;
    }

    Ok(certificate)
}

async fn write_certificate(path: &Path, certificate: &Certificate) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .wrap_err_with(|| format!("creating certificate directory {}", parent.display()))?;
    }
    let content = serde_json::to_vec_pretty(certificate).wrap_err("serializing certificate")?;
    tokio::fs::write(path, content)
        .await
        .wrap_err_with(|| format!("writing certificate {}", path.display()))
}

pub fn certificate_status_for_receipt(receipt: &ExecReceipt) -> CertificateStatus {
    if receipt.status == ExecStatus::Succeeded && !receipt.secrets_read {
        CertificateStatus::Certified
    } else {
        CertificateStatus::Failed
    }
}

fn safe_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{certificate_status_for_receipt, safe_id};
    use crate::receipt::{ExecReceipt, ExecStatus};

    #[test]
    fn receipt_with_secrets_read_is_not_certified_positive() {
        let receipt = ExecReceipt {
            schema_version: 1,
            correlation_id: "secret-receipt".to_string(),
            agent: "test-agent".to_string(),
            model: "test-model".to_string(),
            command: vec!["runner".to_string()],
            status: ExecStatus::Succeeded,
            policy_reason: "allowed".to_string(),
            started_at_unix: 1,
            duration_ms: 1,
            timeout_seconds: 5,
            exit_code: Some(0),
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: true,
            cleanup_attempted: false,
            cleanup_succeeded: false,
        };

        assert_eq!(
            certificate_status_for_receipt(&receipt),
            super::CertificateStatus::Failed
        );
    }

    #[test]
    fn ids_are_path_safe() {
        assert_eq!(safe_id("qwen/code:flash"), "qwen-code-flash");
    }
}
