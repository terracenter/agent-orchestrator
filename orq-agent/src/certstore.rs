use crate::certify::{Certificate, CertificateStatus};
use crate::receipt::receipt_sha256;
use color_eyre::eyre::{eyre, Result, WrapErr};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default)]
pub struct CertificateStore {
    certificates: HashMap<(String, String, String), Certificate>,
    ignored_files: usize,
}

impl CertificateStore {
    pub fn load_dir(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(eyre!(
                "certificate directory {} does not exist",
                path.display()
            ));
        }
        if !path.is_dir() {
            return Err(eyre!(
                "certificate path {} is not a directory",
                path.display()
            ));
        }

        let mut store = Self::default();
        for entry in std::fs::read_dir(path)
            .wrap_err_with(|| format!("reading certificate directory {}", path.display()))?
        {
            let entry = entry.wrap_err("reading certificate directory entry")?;
            let entry_path = entry.path();
            if entry_path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            match load_certificate(&entry_path) {
                Ok(certificate) => store.insert(certificate),
                Err(_) => store.ignored_files += 1,
            }
        }
        Ok(store)
    }

    pub fn lookup(&self, agent: &str, model: &str, task_kind: &str) -> Option<&Certificate> {
        self.certificates
            .get(&(agent.to_string(), model.to_string(), task_kind.to_string()))
    }

    pub fn ignored_files(&self) -> usize {
        self.ignored_files
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.certificates.len()
    }

    fn insert(&mut self, certificate: Certificate) {
        let key = (
            certificate.agent.clone(),
            certificate.model.clone(),
            certificate.task_kind.clone(),
        );
        let replace = self
            .certificates
            .get(&key)
            .map(|current| certificate.created_at_unix >= current.created_at_unix)
            .unwrap_or(true);
        if replace {
            self.certificates.insert(key, certificate);
        }
    }
}

fn load_certificate(path: &Path) -> Result<Certificate> {
    let content = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("reading certificate {}", path.display()))?;
    let certificate: Certificate = serde_json::from_str(&content)
        .wrap_err_with(|| format!("parsing certificate {}", path.display()))?;
    validate_certificate(&certificate)
        .wrap_err_with(|| format!("validating certificate {}", path.display()))?;
    Ok(certificate)
}

fn validate_certificate(certificate: &Certificate) -> Result<()> {
    if certificate.schema_version != 1 {
        return Err(eyre!(
            "unsupported certificate schema_version {}",
            certificate.schema_version
        ));
    }
    if certificate.secrets_read || certificate.receipt.secrets_read {
        return Err(eyre!("certificate indicates secrets were read"));
    }
    if certificate.agent != certificate.receipt.agent
        || certificate.model != certificate.receipt.model
        || certificate.receipt_sha256 != receipt_sha256(&certificate.receipt)?
    {
        return Err(eyre!("certificate does not match embedded receipt"));
    }
    Ok(())
}

pub fn is_certified(certificate: &Certificate) -> bool {
    certificate.status == CertificateStatus::Certified && !certificate.secrets_read
}

pub fn is_failed(certificate: &Certificate) -> bool {
    certificate.status == CertificateStatus::Failed
}

#[cfg(test)]
mod tests {
    use super::CertificateStore;
    use crate::certify::{Certificate, CertificateStatus};
    use crate::receipt::{receipt_sha256, ExecReceipt, ExecStatus};
    use std::fs;

    #[test]
    fn loads_valid_certificates_and_skips_invalid_json() {
        let dir = std::env::temp_dir().join(format!("orq-certs-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("valid.json"),
            cert_json("qwen-code", "m1", "docs", true),
        )
        .unwrap();
        fs::write(dir.join("bad.json"), "not json").unwrap();

        let store = CertificateStore::load_dir(&dir).unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(store.ignored_files(), 1);
        assert!(store.lookup("qwen-code", "m1", "docs").is_some());
        assert!(store.lookup("qwen-code", "m1", "code").is_none());
    }

    #[test]
    fn rejects_secret_read_certificate() {
        let dir = std::env::temp_dir().join(format!("orq-certs-secret-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut cert: Certificate =
            serde_json::from_str(&cert_json("qwen-code", "m1", "docs", true)).unwrap();
        cert.secrets_read = true;
        fs::write(
            dir.join("secret.json"),
            serde_json::to_string(&cert).unwrap(),
        )
        .unwrap();

        let store = CertificateStore::load_dir(&dir).unwrap();
        assert_eq!(store.len(), 0);
        assert_eq!(store.ignored_files(), 1);
    }

    fn cert_json(agent: &str, model: &str, task_kind: &str, certified: bool) -> String {
        let receipt = ExecReceipt {
            schema_version: 1,
            correlation_id: "test".to_string(),
            agent: agent.to_string(),
            model: model.to_string(),
            command: vec!["runner".to_string()],
            status: if certified {
                ExecStatus::Succeeded
            } else {
                ExecStatus::Failed
            },
            policy_reason: "allowed".to_string(),
            started_at_unix: 1,
            duration_ms: 1,
            timeout_seconds: 5,
            exit_code: Some(if certified { 0 } else { 1 }),
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
        };
        let cert = Certificate {
            schema_version: 1,
            certificate_id: format!("cert-{agent}-{model}-{task_kind}"),
            created_at_unix: 1,
            agent: agent.to_string(),
            model: model.to_string(),
            task_kind: task_kind.to_string(),
            status: if certified {
                CertificateStatus::Certified
            } else {
                CertificateStatus::Failed
            },
            receipt_sha256: receipt_sha256(&receipt).unwrap(),
            receipt,
            output_path: None,
            secrets_read: false,
        };
        serde_json::to_string(&cert).unwrap()
    }
}
