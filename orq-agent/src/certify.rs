use crate::adapters::AdaptersRegistry;
use crate::policy::PolicyConfig;
use crate::receipt::{now_unix, ExecReceipt, ExecStatus};
use crate::smoke;
use color_eyre::eyre::{Result, WrapErr};
use serde::Serialize;
use sha2::{Digest, Sha256};
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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
    let receipt_json =
        serde_json::to_vec(&receipt).wrap_err("serializing certification receipt")?;
    let receipt_sha256 = hex_sha256(&receipt_json);
    let created_at_unix = now_unix();
    let certificate_id = format!(
        "cert-{created_at_unix}-{}-{}-{}",
        safe_id(&request.agent),
        safe_id(&request.model),
        safe_id(&request.task_kind)
    );
    let status = if receipt.status == ExecStatus::Succeeded && !receipt.secrets_read {
        CertificateStatus::Certified
    } else {
        CertificateStatus::Failed
    };
    let output_path = request.output.clone();
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
        secrets_read: false,
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

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn safe_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{hex_sha256, safe_id};

    #[test]
    fn hash_is_sha256_hex() {
        assert_eq!(
            hex_sha256(b"orq"),
            "dd9c259677362f1c9bb63eb4cdfdb8a123506fea50c6f4d5c22ecdb36c2f0b52"
        );
    }

    #[test]
    fn ids_are_path_safe() {
        assert_eq!(safe_id("qwen/code:flash"), "qwen-code-flash");
    }
}
