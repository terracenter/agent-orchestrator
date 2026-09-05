use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn fake_runner(name: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join(format!("orq-agent-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path.display().to_string()
}

#[test]
fn detect_supports_external_adapters_registry() {
    let registry = std::env::temp_dir().join(format!(
        "orq-agent-adapters-registry-{}.json",
        std::process::id()
    ));
    fs::write(
        &registry,
        r#"{"schema_version":1,"adapters":[{"name":"custom-agent","binary":"custom-agent-bin","status":"available","argv":["--model","$MODEL","--prompt","$TASK"]}]}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.args([
        "detect",
        "--adapters-config",
        registry.to_str().unwrap(),
        "--format",
        "json",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("custom-agent"))
    .stdout(predicate::str::contains("qwen-code").not())
    .stdout(predicate::str::contains("\"secrets_read\": false"));
}

#[test]
fn exec_supports_external_adapters_registry() {
    let runner = fake_runner(
        "custom-runner",
        "#!/usr/bin/env bash\necho custom-runner-ok\n",
    );
    let task =
        std::env::temp_dir().join(format!("orq-agent-custom-task-{}.md", std::process::id()));
    fs::write(&task, "hello custom").unwrap();
    let registry = std::env::temp_dir().join(format!(
        "orq-agent-custom-registry-{}.json",
        std::process::id()
    ));
    fs::write(
        &registry,
        r#"{"schema_version":1,"adapters":[{"name":"custom-agent","binary":"custom-runner","status":"available","argv":["$MODEL","$TASK"]}]}"#,
    )
    .unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_CUSTOM_AGENT", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "custom-agent",
            "--model",
            "custom-model",
            "--task-file",
            task.to_str().unwrap(),
            "--adapters-config",
            registry.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "5",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"succeeded\""))
        .stdout(predicate::str::contains("custom-runner-ok"));
}

#[test]
fn exec_supports_external_policy_config() {
    let runner = fake_runner("qwen-policy", "#!/usr/bin/env bash\necho should-not-run\n");
    let task =
        std::env::temp_dir().join(format!("orq-agent-policy-task-{}.md", std::process::id()));
    fs::write(&task, "hello policy").unwrap();
    let policy = std::env::temp_dir().join(format!("orq-agent-policy-{}.json", std::process::id()));
    fs::write(
        &policy,
        r#"{"schema_version":1,"approval_required_model_patterns":["max"],"blocked_adapter_statuses":["deprecated_or_quarantine"],"gated_adapter_statuses":["gated"]}"#,
    )
    .unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--policy-config",
            policy.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "5",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"blocked\""))
        .stdout(predicate::str::contains(
            "model qwen3.8-max requires explicit human approval",
        ))
        .stdout(predicate::str::contains("should-not-run").not());
}

#[test]
fn exec_qwen_fake_succeeds_with_receipt() {
    let runner = fake_runner("qwen-ok", "#!/usr/bin/env bash\necho fake-qwen-ok\n");
    let task = std::env::temp_dir().join(format!("orq-agent-task-{}.md", std::process::id()));
    fs::write(&task, "hello fake").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "5",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"succeeded\""))
        .stdout(predicate::str::contains("fake-qwen-ok"))
        .stdout(predicate::str::contains("hello fake").not())
        .stdout(predicate::str::contains("<task 10 bytes>"));
}

#[test]
fn models_qwen_reports_candidate_without_secrets() {
    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.args(["models", "--agent", "qwen-code", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("qwen3.8-max"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));
}

#[test]
fn orq_binary_alias_supports_models_command() {
    let mut cmd = Command::cargo_bin("orq").unwrap();
    cmd.args(["models", "--agent", "qwen-code", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("qwen3.8-max"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));
}

#[test]
fn state_status_creates_temp_db_without_secrets() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "state",
            "status",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": 4"))
        .stdout(predicate::str::contains("\"secrets_read\": false"))
        .stdout(predicate::str::contains("agents"))
        .stdout(predicate::str::contains("models"));
}

#[test]
fn discover_reports_sources_and_writes_temp_state_without_secrets() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "discover",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"config_source\""))
        .stdout(predicate::str::contains("\"state_source\""))
        .stdout(predicate::str::contains("\"secrets_read\": false"))
        .stdout(predicate::str::contains(
            "orq-agent/config/adapters-registry.json",
        ))
        .stdout(predicate::str::contains(
            "orq-agent/config/models-catalog.json",
        ));

    assert!(db.exists());
}

#[test]
fn route_default_config_reports_documentation_without_secrets() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "route",
            "--task-kind",
            "documentation",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"task_kind\": \"documentation\""))
        .stdout(predicate::str::contains("\"secrets_read\": false"))
        .stdout(predicate::str::contains(
            "orq-agent/config/routing-matrix.json",
        ));
}

#[test]
fn route_uses_certificate_directory_for_exact_match() {
    let runner = fake_runner(
        "qwen-route-cert",
        "#!/usr/bin/env bash\necho 'ORQ_SMOKE_OK agent=qwen-code model=qwen3.6-flash'\n",
    );
    let cert_dir =
        std::env::temp_dir().join(format!("orq-agent-route-certs-{}", std::process::id()));
    fs::create_dir_all(&cert_dir).unwrap();
    let output = cert_dir.join("qwen-docs.json");
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut certify = Command::cargo_bin("orq-agent").unwrap();
    certify
        .env("ORQ_AGENT_BIN_QWEN_CODE", &runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "certify",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.6-flash",
            "--task-kind",
            "documentation",
            "--timeout",
            "5",
            "--output",
            output.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let mut route = Command::cargo_bin("orq-agent").unwrap();
    route
        .env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "route",
            "--task-kind",
            "documentation",
            "--cert-dir",
            cert_dir.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"certificate_store_used\": true"))
        .stdout(predicate::str::contains(
            "\"preferred_certificate\": \"cert-",
        ))
        .stdout(predicate::str::contains("certified:"));
}

#[test]
fn route_supports_external_config() {
    let config =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/routing-matrix.json");
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "route",
            "--task-kind",
            "documentation",
            "--config",
            config.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("routing-matrix.json"))
        .stdout(predicate::str::contains(
            "\"default_model\": \"qwen3.6-flash\"",
        ));
}

#[test]
fn certify_qwen_fake_writes_certificate() {
    let runner = fake_runner(
        "qwen-certify",
        "#!/usr/bin/env bash\necho 'ORQ_SMOKE_OK agent=qwen-code model=qwen3.8-max'\n",
    );
    let output = std::env::temp_dir().join(format!("orq-agent-cert-{}.json", std::process::id()));
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "certify",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-kind",
            "documentation",
            "--timeout",
            "5",
            "--output",
            output.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"certified\""))
        .stdout(predicate::str::contains("\"receipt_sha256\""));

    let certificate = fs::read_to_string(output).unwrap();
    assert!(certificate.contains("qwen3.8-max"));
    assert!(certificate.contains("documentation"));
}

#[test]
fn smoke_qwen_fake_succeeds_with_receipt() {
    let runner = fake_runner(
        "qwen-smoke",
        "#!/usr/bin/env bash\necho 'ORQ_SMOKE_OK agent=qwen-code model=qwen3.8-max'\n",
    );
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "smoke",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--timeout",
            "5",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"succeeded\""))
        .stdout(predicate::str::contains("ORQ_SMOKE_OK"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));
}

#[test]
fn exec_unknown_agent_returns_invalid_request_receipt() {
    let task =
        std::env::temp_dir().join(format!("orq-agent-unknown-task-{}.md", std::process::id()));
    fs::write(&task, "hello unknown").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "missing-agent",
            "--model",
            "missing-model",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "5",
            "--correlation-id",
            "test-correlation",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"invalid_request\""))
        .stdout(predicate::str::contains(
            "unknown agent adapter: missing-agent",
        ))
        .stdout(predicate::str::contains("test-correlation"));
}

#[test]
fn exec_rejects_timeout_above_ceiling_as_receipt() {
    let task = std::env::temp_dir().join(format!(
        "orq-agent-timeout-ceiling-task-{}.md",
        std::process::id()
    ));
    fs::write(&task, "hello timeout ceiling").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "301",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"invalid_request\""))
        .stdout(predicate::str::contains(
            "timeout must be between 1 and 300 seconds",
        ));
}

#[test]
fn exec_missing_binary_returns_spawn_failed_receipt() {
    let task = std::env::temp_dir().join(format!("orq-agent-spawn-task-{}.md", std::process::id()));
    fs::write(&task, "hello spawn").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env(
        "ORQ_AGENT_BIN_QWEN_CODE",
        "/tmp/orq-agent-definitely-missing-binary",
    )
    .env("ORQ_STATE_DB", &db)
    .args([
        "exec",
        "--agent",
        "qwen-code",
        "--model",
        "qwen3.8-max",
        "--task-file",
        task.to_str().unwrap(),
        "--db-path",
        db.to_str().unwrap(),
        "--timeout",
        "5",
        "--format",
        "json",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"status\": \"spawn_failed\""))
    .stdout(predicate::str::contains("spawning agent qwen-code"));
}

#[test]
fn exec_timeout_preserves_partial_stdout_tail() {
    let runner = fake_runner(
        "qwen-partial",
        "#!/usr/bin/env bash\necho partial-before-timeout\nsleep 5\n",
    );
    let task =
        std::env::temp_dir().join(format!("orq-agent-partial-task-{}.md", std::process::id()));
    fs::write(&task, "hello partial").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"timed_out\""))
        .stdout(predicate::str::contains("partial-before-timeout"));
}

#[test]
fn exec_output_tail_is_bounded_to_recent_output() {
    let runner = fake_runner("qwen-long-output", "#!/usr/bin/env bash\npython3 - <<'PY'\nprint('A' * 20000)\nprint('RECENT-TAIL-MARKER')\nPY\n");
    let task = std::env::temp_dir().join(format!(
        "orq-agent-long-output-task-{}.md",
        std::process::id()
    ));
    fs::write(&task, "hello long output").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "5",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("RECENT-TAIL-MARKER"));
}

#[test]
fn exec_timeout_kills_child_process_group() {
    let marker =
        std::env::temp_dir().join(format!("orq-agent-orphan-marker-{}", std::process::id()));
    let body = format!(
        "#!/usr/bin/env bash\n(sleep 2; echo orphan-alive > '{}') &\nsleep 10\n",
        marker.display()
    );
    let runner = fake_runner("qwen-process-group", &body);
    let task = std::env::temp_dir().join(format!("orq-agent-pgid-task-{}.md", std::process::id()));
    fs::write(&task, "hello pgid").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"timed_out\""))
        .stdout(predicate::str::contains("\"cleanup_attempted\": true"))
        .stdout(predicate::str::contains("\"cleanup_succeeded\": true"));

    std::thread::sleep(std::time::Duration::from_secs(3));
    assert!(!marker.exists(), "timeout left an orphaned process behind");
}

#[test]
fn exec_pgid_cleanup() {
    let marker = std::env::temp_dir().join(format!(
        "orq-agent-pgid-cleanup-marker-{}",
        std::process::id()
    ));
    let body = format!(
        "#!/usr/bin/env bash\ntrap '' TERM\n(sleep 2; echo child-alive > '{}') &\nwhile true; do sleep 1; done\n",
        marker.display()
    );
    let runner = fake_runner("qwen-pgid-cleanup-runner", &body);
    let task = std::env::temp_dir().join(format!(
        "orq-agent-pgid-cleanup-task-{}.md",
        std::process::id()
    ));
    fs::write(&task, "hello pgid cleanup").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"timed_out\""))
        .stdout(predicate::str::contains("\"cleanup_attempted\": true"))
        .stdout(predicate::str::contains("\"cleanup_succeeded\": true"));

    std::thread::sleep(std::time::Duration::from_millis(2500));
    assert!(
        !marker.exists(),
        "PGID cleanup failed: orphaned background child process survived"
    );
}

#[test]
fn exec_qwen_fake_timeout_is_reported() {
    let runner = fake_runner("qwen-sleep", "#!/usr/bin/env bash\nsleep 5\n");
    let task =
        std::env::temp_dir().join(format!("orq-agent-timeout-task-{}.md", std::process::id()));
    fs::write(&task, "hello timeout").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"timed_out\""));
}

#[test]
fn exec_without_correlation_id_uses_unique_fallback() {
    let task =
        std::env::temp_dir().join(format!("orq-agent-fallback-task-{}.md", std::process::id()));
    fs::write(&task, "hello fallback").unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "exec",
            "--agent",
            "missing-agent",
            "--model",
            "missing-model",
            "--task-file",
            task.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "0",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r#""correlation_id": "orq-agent-\d{16,}-\d+""#).unwrap());
}

#[test]
fn certify_without_correlation_id_uses_unique_fallback() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "certify",
            "--agent",
            "missing-agent",
            "--model",
            "missing-model",
            "--task-kind",
            "regression",
            "--db-path",
            db.to_str().unwrap(),
            "--timeout",
            "0",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::is_match(
                r#""certificate_id": "cert-\d{16,}-\d+-missing-agent-missing-model-regression""#,
            )
            .unwrap(),
        );
}

#[test]
fn quota_cli_help_is_visible() {
    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.args(["quota", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Manage and report agent provider quotas",
        ))
        .stdout(predicate::str::contains("record"))
        .stdout(predicate::str::contains("report"));
}

#[test]
fn quota_cli_record_manual_and_report() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut record_cmd = Command::cargo_bin("orq-agent").unwrap();
    record_cmd
        .env("ORQ_STATE_DB", &db)
        .args([
            "quota",
            "record",
            "--provider",
            "agy",
            "--scope",
            "gemini-weekly",
            "--remaining-pct",
            "47.17",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"agy\""))
        .stdout(predicate::str::contains("\"remaining_pct\": 47.17"))
        .stdout(predicate::str::contains("\"used_pct\": 52.83"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));

    let mut report_cmd = Command::cargo_bin("orq-agent").unwrap();
    report_cmd
        .env("ORQ_STATE_DB", &db)
        .args(["quota", "report", "--provider", "agy", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"agy\""))
        .stdout(predicate::str::contains("\"gemini-weekly\""))
        .stdout(predicate::str::contains("47.17"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));
}

#[test]
fn quota_cli_manual_record_without_percentages_derives_quota_unknown() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    // Manual record without percentages or status override
    let mut record_cmd = Command::cargo_bin("orq-agent").unwrap();
    record_cmd
        .env("ORQ_STATE_DB", &db)
        .args([
            "quota",
            "record",
            "--provider",
            "qwen",
            "--scope",
            "general",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"qwen\""))
        .stdout(predicate::str::contains("\"scope\": \"general\""))
        .stdout(predicate::str::contains("\"status\": \"quota_unknown\""))
        .stdout(predicate::str::contains("\"remaining_pct\": null"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));

    let mut report_cmd = Command::cargo_bin("orq-agent").unwrap();
    report_cmd
        .env("ORQ_STATE_DB", &db)
        .args(["quota", "report", "--provider", "qwen", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"qwen\""))
        .stdout(predicate::str::contains("\"status\": \"quota_unknown\""))
        .stdout(predicate::str::contains("\"scope\": \"general\""))
        .stdout(predicate::str::contains("\"status\": \"quota_unknown\""));
}

#[test]
fn quota_cli_record_json_array_and_report_with_resets() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let json_payload = r#"[
        {"provider": "agy", "scope": "gemini-weekly", "remaining_pct": 47.17},
        {"provider": "agy", "scope": "gemini-five-hour", "remaining_pct": 96.48},
        {"provider": "agy", "scope": "claude-gpt-weekly", "remaining_pct": 62.69},
        {"provider": "agy", "scope": "claude-gpt-five-hour", "remaining_pct": 0.0, "captured_at_unix": 1700000000, "reset_in_seconds": 9180, "status": "exhausted"},
        {"provider": "claude-code", "scope": "session", "used_pct": 30.0},
        {"provider": "claude-code", "scope": "weekly", "used_pct": 3.0, "metadata": {"promo": "+50% until Sep 13"}},
        {"provider": "codex", "scope": "short-term", "remaining_pct": 22.0, "captured_at_unix": 1700000000, "reset_in_seconds": 14760},
        {"provider": "codex", "scope": "long-term", "remaining_pct": 80.0, "captured_at_unix": 1700000000, "reset_in_seconds": 440640}
    ]"#;

    let mut record_cmd = Command::cargo_bin("orq-agent").unwrap();
    record_cmd
        .args([
            "quota",
            "record",
            "--json",
            json_payload,
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": 1"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));

    let mut report_cmd = Command::cargo_bin("orq-agent").unwrap();
    report_cmd
        .args([
            "quota",
            "report",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"agy\""))
        .stdout(predicate::str::contains("\"provider\": \"claude-code\""))
        .stdout(predicate::str::contains("\"provider\": \"codex\""))
        .stdout(predicate::str::contains("\"provider\": \"qwen\""))
        .stdout(predicate::str::contains("\"status\": \"quota_unknown\""))
        .stdout(predicate::str::contains("claude-gpt-five-hour"))
        .stdout(predicate::str::contains("\"reset_at_unix\": 1700009180"))
        .stdout(predicate::str::contains("\"reset_at_unix\": 1700014760"))
        .stdout(predicate::str::contains("\"reset_at_unix\": 1700440640"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));
}

#[test]
fn quota_cli_normalizes_provider_case_and_aggregates_partial_unknown() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    // Record scope 1 with uppercase provider "AGY"
    let mut record_cmd1 = Command::cargo_bin("orq-agent").unwrap();
    record_cmd1
        .args([
            "quota",
            "record",
            "--provider",
            "AGY",
            "--scope",
            "gemini-weekly",
            "--remaining-pct",
            "80.0",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"agy\""));

    // Record scope 2 with lowercase provider and no percentages -> quota_unknown
    let mut record_cmd2 = Command::cargo_bin("orq-agent").unwrap();
    record_cmd2
        .args([
            "quota",
            "record",
            "--provider",
            "agy",
            "--scope",
            "custom-unknown",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"agy\""))
        .stdout(predicate::str::contains("\"status\": \"quota_unknown\""));

    // Report filtering with uppercase "AGY" -> should match lowercase "agy",
    // and since one scope is quota_unknown, aggregate status must be quota_unknown
    let mut report_cmd = Command::cargo_bin("orq-agent").unwrap();
    report_cmd
        .args([
            "quota",
            "report",
            "--provider",
            "AGY",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"agy\""))
        .stdout(predicate::str::contains("\"status\": \"quota_unknown\""))
        .stdout(predicate::str::contains("\"scope\": \"gemini-weekly\""))
        .stdout(predicate::str::contains("\"scope\": \"custom-unknown\""));
}

#[test]
fn quota_cli_qwen_without_detector_reports_quota_unknown() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut report_cmd = Command::cargo_bin("orq-agent").unwrap();
    report_cmd
        .args([
            "quota",
            "report",
            "--provider",
            "qwen",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"provider\": \"qwen\""))
        .stdout(predicate::str::contains("\"status\": \"quota_unknown\""))
        .stdout(predicate::str::contains("\"scopes\": []"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));
}

#[test]
fn quota_cli_db_path_precedence_over_env_var() {
    let state_dir = tempfile::tempdir().unwrap();
    let env_db = state_dir.path().join("env_state.sqlite");
    let override_db = state_dir.path().join("override_state.sqlite");

    let mut record_cmd = Command::cargo_bin("orq-agent").unwrap();
    record_cmd
        .env("ORQ_STATE_DB", &env_db)
        .args([
            "quota",
            "record",
            "--provider",
            "agy",
            "--scope",
            "gemini-weekly",
            "--remaining-pct",
            "99.0",
            "--db-path",
            override_db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    // Verify record was written to override_db, not env_db
    assert!(override_db.exists(), "override_db must exist");

    let mut report_override = Command::cargo_bin("orq-agent").unwrap();
    report_override
        .args([
            "quota",
            "report",
            "--provider",
            "agy",
            "--db-path",
            override_db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("99.0"));

    // If we query env_db (which shouldn't have any records), agy will have empty scopes
    let mut report_env = Command::cargo_bin("orq-agent").unwrap();
    report_env
        .args([
            "quota",
            "report",
            "--provider",
            "agy",
            "--db-path",
            env_db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"scopes\": []"));
}

#[test]
fn quota_cli_migration_idempotent_on_existing_db() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    // Open first time (migrates to version 3)
    let mut status_cmd = Command::cargo_bin("orq-agent").unwrap();
    status_cmd
        .args([
            "state",
            "status",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": 4"))
        .stdout(predicate::str::contains("quota_snapshots"));

    // Migrate again explicitly
    let mut migrate_cmd = Command::cargo_bin("orq-agent").unwrap();
    migrate_cmd
        .args([
            "state",
            "migrate",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": 4"));
}

#[test]
fn route_cli_avoids_five_hour_exhausted_scope() {
    let runner_agy = fake_runner("agy-route", "#!/usr/bin/env bash\necho agy-ok\n");
    let runner_qwen = fake_runner("qwen-route", "#!/usr/bin/env bash\necho qwen-ok\n");
    let runner_claude = fake_runner("claude-route", "#!/usr/bin/env bash\necho claude-ok\n");

    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut record_cmd = Command::cargo_bin("orq-agent").unwrap();
    record_cmd
        .args([
            "quota",
            "record",
            "--provider",
            "agy",
            "--scope",
            "gemini-five-hour",
            "--remaining-pct",
            "0.0",
            "--status",
            "exhausted",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let mut record_qwen = Command::cargo_bin("orq-agent").unwrap();
    record_qwen
        .args([
            "quota",
            "record",
            "--provider",
            "qwen",
            "--scope",
            "general",
            "--remaining-pct",
            "80.0",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let mut route_cmd = Command::cargo_bin("orq-agent").unwrap();
    route_cmd
        .env("ORQ_AGENT_BIN_AGY", runner_agy)
        .env("ORQ_AGENT_BIN_QWEN_CODE", runner_qwen)
        .env("ORQ_AGENT_BIN_CLAUDE_CODE", runner_claude)
        .args([
            "route",
            "--task-kind",
            "debugging",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"selected_agent\": \"qwen-code\"",
        ))
        .stdout(predicate::str::contains(
            "\"selected_model\": \"qwen3.6-flash\"",
        ))
        .stdout(predicate::str::contains("\"fallback_applied\": true"));
}

#[test]
fn route_cli_prefers_gated_with_allow_gated_when_weekly_quota_high() {
    let runner_agy = fake_runner("agy-route-gated", "#!/usr/bin/env bash\necho agy-ok\n");
    let runner_qwen = fake_runner("qwen-route-gated", "#!/usr/bin/env bash\necho qwen-ok\n");
    let runner_claude = fake_runner(
        "claude-route-gated",
        "#!/usr/bin/env bash\necho claude-ok\n",
    );

    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    // Default AGY is exhausted
    let mut record_agy = Command::cargo_bin("orq-agent").unwrap();
    record_agy
        .args([
            "quota",
            "record",
            "--provider",
            "agy",
            "--scope",
            "gemini-five-hour",
            "--remaining-pct",
            "0.0",
            "--status",
            "exhausted",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    // Cheap Qwen is also exhausted
    let mut record_qwen = Command::cargo_bin("orq-agent").unwrap();
    record_qwen
        .args([
            "quota",
            "record",
            "--provider",
            "qwen",
            "--scope",
            "general",
            "--remaining-pct",
            "0.0",
            "--status",
            "exhausted",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    // Gated Claude has healthy weekly quota
    let mut record_claude = Command::cargo_bin("orq-agent").unwrap();
    record_claude
        .args([
            "quota",
            "record",
            "--provider",
            "claude-code",
            "--scope",
            "weekly",
            "--remaining-pct",
            "85.0",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let mut route_cmd = Command::cargo_bin("orq-agent").unwrap();
    route_cmd
        .env("ORQ_AGENT_BIN_AGY", runner_agy)
        .env("ORQ_AGENT_BIN_QWEN_CODE", runner_qwen)
        .env("ORQ_AGENT_BIN_CLAUDE_CODE", runner_claude)
        .args([
            "route",
            "--task-kind",
            "debugging",
            "--allow-gated",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"selected_agent\": \"claude-code\"",
        ))
        .stdout(predicate::str::contains(
            "\"selected_model\": \"claude-sonnet-5\"",
        ))
        .stdout(predicate::str::contains("\"fallback_applied\": true"));
}

#[test]
fn route_cli_quota_unknown_does_not_penalize() {
    let runner_agy = fake_runner("agy-route-unk", "#!/usr/bin/env bash\necho agy-ok\n");
    let runner_qwen = fake_runner("qwen-route-unk", "#!/usr/bin/env bash\necho qwen-ok\n");

    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut record_cmd = Command::cargo_bin("orq-agent").unwrap();
    record_cmd
        .args([
            "quota",
            "record",
            "--provider",
            "qwen",
            "--scope",
            "general",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let mut record_agy = Command::cargo_bin("orq-agent").unwrap();
    record_agy
        .args([
            "quota",
            "record",
            "--provider",
            "agy",
            "--scope",
            "gemini-weekly",
            "--remaining-pct",
            "95.0",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let mut route_cmd = Command::cargo_bin("orq-agent").unwrap();
    route_cmd
        .env("ORQ_AGENT_BIN_AGY", runner_agy)
        .env("ORQ_AGENT_BIN_QWEN_CODE", runner_qwen)
        .args([
            "route",
            "--task-kind",
            "documentation",
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"selected_agent\": \"qwen-code\"",
        ))
        .stdout(predicate::str::contains(
            "\"selected_model\": \"qwen3.6-flash\"",
        ))
        .stdout(predicate::str::contains("\"fallback_applied\": false"));
}

#[test]
fn compliance_cli_help() {
    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.args(["compliance", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Audit agent compliance for workspace rules (rtk, engram, vg)"))
        .stdout(predicate::str::contains("--rtk-usage"))
        .stdout(predicate::str::contains("--engram-summary"))
        .stdout(predicate::str::contains("--vg-sync"));
}

#[test]
fn compliance_cli_rtk_usage_violation_and_ok() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let log_viol = state_dir.path().join("raw.log");
    fs::write(&log_viol, "git status\n").unwrap();

    let mut cmd_viol = Command::cargo_bin("orq-agent").unwrap();
    cmd_viol
        .env("ORQ_STATE_DB", &db)
        .args([
            "compliance",
            "--rtk-usage",
            "--log",
            log_viol.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("\"status\": \"violation\""))
        .stdout(predicate::str::contains("\"raw_invocations_count\": 1"));

    let log_ok = state_dir.path().join("rtk.log");
    fs::write(&log_ok, "rtk git status\n").unwrap();

    let mut cmd_ok = Command::cargo_bin("orq-agent").unwrap();
    cmd_ok
        .env("ORQ_STATE_DB", &db)
        .args([
            "compliance",
            "--rtk-usage",
            "--log",
            log_ok.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"raw_invocations_count\": 0"));
}

#[test]
fn compliance_cli_engram_summary_ok_and_violation() {
    let fake_engram_ok = fake_runner(
        "fake-engram-ok",
        "#!/usr/bin/env bash\nTODAY=$(date +%Y-%m-%d)\necho \"[1] #42 (session_summary) — Session summary\"\necho \"    ${TODAY} 17:00:00 | project: test-proj | scope: project\"\n",
    );
    let fake_engram_empty = fake_runner(
        "fake-engram-empty",
        "#!/usr/bin/env bash\necho 'no summaries found'\n",
    );

    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd_ok = Command::cargo_bin("orq-agent").unwrap();
    cmd_ok
        .env("ORQ_STATE_DB", &db)
        .args([
            "compliance",
            "--engram-summary",
            "--project",
            "test-proj",
            "--engram-bin",
            &fake_engram_ok,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"session_summaries_count\": 1"));

    let mut cmd_viol = Command::cargo_bin("orq-agent").unwrap();
    cmd_viol
        .env("ORQ_STATE_DB", &db)
        .args([
            "compliance",
            "--engram-summary",
            "--project",
            "test-proj",
            "--engram-bin",
            &fake_engram_empty,
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("\"status\": \"violation\""))
        .stdout(predicate::str::contains("\"session_summaries_count\": 0"));
}

#[test]
fn compliance_cli_vg_sync_stale_when_vault_newer() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let vault_dir = tempfile::tempdir().unwrap();
    let git_refs = vault_dir.path().join(".git/refs/heads");
    fs::create_dir_all(&git_refs).unwrap();
    fs::write(vault_dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let main_ref = git_refs.join("main");
    fs::write(&main_ref, "commit-sha\n").unwrap();

    let kuzu_dir = tempfile::tempdir().unwrap();
    let kuzu_marker = kuzu_dir.path().join("vault.kuzu.sync");
    fs::write(&kuzu_marker, "sync-marker\n").unwrap();

    // Set kuzu marker timestamp to older time so vault ref is newer
    let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(10);
    let times = std::fs::FileTimes::new().set_modified(old_time);
    let file = std::fs::File::options().write(true).open(&kuzu_marker).unwrap();
    let _ = file.set_times(times);

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "compliance",
            "--vg-sync",
            "--vault-path",
            vault_dir.path().to_str().unwrap(),
            "--kuzu-path",
            kuzu_marker.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("\"status\": \"violation\""))
        .stdout(predicate::str::contains("\"is_fresh\": false"));
}

#[test]
fn compliance_cli_vg_sync_fresh() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let vault_dir = tempfile::tempdir().unwrap();
    let git_refs = vault_dir.path().join(".git/refs/heads");
    fs::create_dir_all(&git_refs).unwrap();
    fs::write(vault_dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let main_ref = git_refs.join("main");
    fs::write(&main_ref, "commit-sha\n").unwrap();

    // Set vault ref timestamp to older time so kuzu marker is newer
    let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(10);
    let times = std::fs::FileTimes::new().set_modified(old_time);
    let file = std::fs::File::options().write(true).open(&main_ref).unwrap();
    let _ = file.set_times(times);

    let kuzu_dir = tempfile::tempdir().unwrap();
    let kuzu_marker = kuzu_dir.path().join("vault.kuzu.sync");
    fs::write(&kuzu_marker, "sync-marker\n").unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .args([
            "compliance",
            "--vg-sync",
            "--vault-path",
            vault_dir.path().to_str().unwrap(),
            "--kuzu-path",
            kuzu_marker.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"is_fresh\": true"));
}

#[test]
fn compliance_cli_vg_sync_not_available() {
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_STATE_DB", &db)
        .env_remove("ORQ_VAULT_PATH")
        .env_remove("VAULT_PATH")
        .env_remove("ORQ_KUZU_PATH")
        .env_remove("KUZU_PATH")
        .args([
            "compliance",
            "--vg-sync",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"not_available\""))
        .stdout(predicate::str::contains("\"is_fresh\": false"));
}

#[test]
fn models_refresh_cli_merges_feed_idempotently() {
    let temp_dir = tempfile::tempdir().unwrap();
    let catalog_path = temp_dir.path().join("models-catalog.json");
    let feed_path = temp_dir.path().join("market-feed.json");

    let initial_catalog = r#"{
        "schema_version": 1,
        "agents": {
            "qwen-code": [
                {
                    "id": "qwen3.6-flash",
                    "source": "manual",
                    "confidence": "baseline",
                    "notes": "initial candidate"
                }
            ]
        }
    }"#;
    fs::write(&catalog_path, initial_catalog).unwrap();

    let feed_content = r#"{
        "schema_version": 1,
        "feed_source": "test_feed_source",
        "agents": {
            "qwen-code": [
                {
                    "id": "qwen3.6-flash",
                    "promo": "special 50% weekly usage limit",
                    "cost_hint": 0.0001
                },
                {
                    "id": "qwen3.8-max",
                    "cost_hint": 0.005,
                    "status": "active",
                    "notes": "newly added model"
                },
                {
                    "id": "old-deprecated-model",
                    "status": "deprecated"
                }
            ]
        }
    }"#;
    fs::write(&feed_path, feed_content).unwrap();

    // First execution: merges feed into catalog
    let mut cmd1 = Command::cargo_bin("orq-agent").unwrap();
    cmd1.args([
        "models",
        "refresh",
        "--feed",
        feed_path.to_str().unwrap(),
        "--catalog",
        catalog_path.to_str().unwrap(),
        "--format",
        "json",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"schema_version\": 2"))
    .stdout(predicate::str::contains("\"added\": 1"))
    .stdout(predicate::str::contains("\"updated\": 1"))
    .stdout(predicate::str::contains("\"deprecated\": 1"))
    .stdout(predicate::str::contains("\"total_models\": 3"))
    .stdout(predicate::str::contains("\"fetched_at\":"));

    // Verify catalog on disk was updated
    let updated_catalog = fs::read_to_string(&catalog_path).unwrap();
    assert!(updated_catalog.contains("special 50% weekly usage limit"));
    assert!(updated_catalog.contains("qwen3.8-max"));
    assert!(updated_catalog.contains("old-deprecated-model"));
    assert!(updated_catalog.contains("\"schema_version\": 2"));

    // Second execution: must be idempotent (0 added, 0 updated, 0 deprecated)
    let mut cmd2 = Command::cargo_bin("orq-agent").unwrap();
    cmd2.args([
        "models",
        "refresh",
        "--feed",
        feed_path.to_str().unwrap(),
        "--catalog",
        catalog_path.to_str().unwrap(),
        "--format",
        "json",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"added\": 0"))
    .stdout(predicate::str::contains("\"updated\": 0"))
    .stdout(predicate::str::contains("\"deprecated\": 0"))
    .stdout(predicate::str::contains("\"total_models\": 3"));
}

#[test]
fn route_cli_fallback_when_top_model_status_down() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = temp_dir.path().join("state.sqlite");
    let config_path = temp_dir.path().join("routing-matrix.json");
    let adapters_path = temp_dir.path().join("adapters-registry.json");
    let models_path = temp_dir.path().join("models-catalog.json");

    let routing_config = r#"{
        "schema_version": 1,
        "approval_required_model_patterns": ["opus"],
        "routes": [
            {
                "task_kind": "review",
                "default_agent": "primary-agent",
                "default_model": "top-model",
                "cheap_sufficient": "fallback-agent/second-model",
                "escalate_to": "none",
                "avoid": [],
                "rationale": "testing fallback when top model status is down"
            }
        ]
    }"#;
    fs::write(&config_path, routing_config).unwrap();

    let adapters_registry = r#"{
        "schema_version": 1,
        "adapters": [
            {
                "name": "primary-agent",
                "binary": "primary-runner",
                "status": "available",
                "argv": ["$MODEL", "$TASK"]
            },
            {
                "name": "fallback-agent",
                "binary": "fallback-runner",
                "status": "available",
                "argv": ["$MODEL", "$TASK"]
            }
        ]
    }"#;
    fs::write(&adapters_path, adapters_registry).unwrap();

    let models_catalog = r#"{
        "schema_version": 2,
        "agents": {
            "primary-agent": [
                {
                    "id": "top-model",
                    "source": "catalog",
                    "confidence": "high",
                    "notes": "unhealthy top model",
                    "status": "down"
                }
            ],
            "fallback-agent": [
                {
                    "id": "second-model",
                    "source": "catalog",
                    "confidence": "high",
                    "notes": "healthy fallback model",
                    "status": "active"
                }
            ]
        }
    }"#;
    fs::write(&models_path, models_catalog).unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_PRIMARY_AGENT", "primary-runner")
        .env("ORQ_AGENT_BIN_FALLBACK_AGENT", "fallback-runner")
        .env("ORQ_STATE_DB", &db)
        .args([
            "route",
            "--task-kind",
            "review",
            "--config",
            config_path.to_str().unwrap(),
            "--adapters-config",
            adapters_path.to_str().unwrap(),
            "--models-config",
            models_path.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"selected_agent\": \"fallback-agent\""))
        .stdout(predicate::str::contains("\"selected_model\": \"second-model\""))
        .stdout(predicate::str::contains("\"fallback_applied\": true"));
}

#[test]
fn route_cli_prefers_promo_on_equal_cost_and_preserves_cheap_over_expensive_promo() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = temp_dir.path().join("state.sqlite");
    let config_path = temp_dir.path().join("routing-matrix.json");
    let adapters_path = temp_dir.path().join("adapters-registry.json");
    let models_path = temp_dir.path().join("models-catalog.json");

    let routing_config = r#"{
        "schema_version": 1,
        "approval_required_model_patterns": ["opus"],
        "routes": [
            {
                "task_kind": "equal_cost",
                "default_agent": "agent-plain",
                "default_model": "model-plain",
                "cheap_sufficient": "agent-promo/model-promo",
                "escalate_to": "none",
                "avoid": [],
                "rationale": "promo preferred over equivalent cost"
            },
            {
                "task_kind": "cheap_vs_expensive_promo",
                "default_agent": "cheap-agent",
                "default_model": "cheap-model",
                "cheap_sufficient": "none",
                "escalate_to": "expensive-agent/expensive-promo-model",
                "avoid": [],
                "rationale": "cheap healthy not displaced by expensive candidate with promo"
            }
        ]
    }"#;
    fs::write(&config_path, routing_config).unwrap();

    let adapters_registry = r#"{
        "schema_version": 1,
        "adapters": [
            {"name": "agent-plain", "binary": "runner", "status": "available", "argv": ["$MODEL", "$TASK"]},
            {"name": "agent-promo", "binary": "runner", "status": "available", "argv": ["$MODEL", "$TASK"]},
            {"name": "cheap-agent", "binary": "runner", "status": "available", "argv": ["$MODEL", "$TASK"]},
            {"name": "expensive-agent", "binary": "runner", "status": "available", "argv": ["$MODEL", "$TASK"]}
        ]
    }"#;
    fs::write(&adapters_path, adapters_registry).unwrap();

    let models_catalog = r#"{
        "schema_version": 2,
        "agents": {
            "agent-plain": [
                {"id": "model-plain", "source": "s", "confidence": "c", "notes": "n", "cost_hint": 0.002, "status": "active"}
            ],
            "agent-promo": [
                {"id": "model-promo", "source": "s", "confidence": "c", "notes": "n", "cost_hint": 0.002, "promo": "+50% usage promo", "status": "active"}
            ],
            "cheap-agent": [
                {"id": "cheap-model", "source": "s", "confidence": "c", "notes": "n", "cost_hint": 0.0001, "status": "active"}
            ],
            "expensive-agent": [
                {"id": "expensive-promo-model", "source": "s", "confidence": "c", "notes": "n", "cost_hint": 0.01, "promo": "+50% promo", "status": "active"}
            ]
        }
    }"#;
    fs::write(&models_path, models_catalog).unwrap();

    // 1. Equal cost: agent-promo/model-promo is preferred over agent-plain/model-plain
    let mut cmd1 = Command::cargo_bin("orq-agent").unwrap();
    cmd1.env("ORQ_AGENT_BIN_AGENT_PLAIN", "runner")
        .env("ORQ_AGENT_BIN_AGENT_PROMO", "runner")
        .env("ORQ_STATE_DB", &db)
        .args([
            "route",
            "--task-kind",
            "equal_cost",
            "--config",
            config_path.to_str().unwrap(),
            "--adapters-config",
            adapters_path.to_str().unwrap(),
            "--models-config",
            models_path.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"selected_agent\": \"agent-promo\""))
        .stdout(predicate::str::contains("\"selected_model\": \"model-promo\""))
        .stdout(predicate::str::contains("\"fallback_applied\": true"));

    // 2. Cheap vs expensive with promo: cheap-agent/cheap-model is kept (not displaced by expensive model with promo)
    let mut cmd2 = Command::cargo_bin("orq-agent").unwrap();
    cmd2.env("ORQ_AGENT_BIN_CHEAP_AGENT", "runner")
        .env("ORQ_AGENT_BIN_EXPENSIVE_AGENT", "runner")
        .env("ORQ_STATE_DB", &db)
        .args([
            "route",
            "--task-kind",
            "cheap_vs_expensive_promo",
            "--config",
            config_path.to_str().unwrap(),
            "--adapters-config",
            adapters_path.to_str().unwrap(),
            "--models-config",
            models_path.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"selected_agent\": \"cheap-agent\""))
        .stdout(predicate::str::contains("\"selected_model\": \"cheap-model\""))
        .stdout(predicate::str::contains("\"fallback_applied\": false"));
}

#[test]
fn delegate_cli_help() {
    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.args(["delegate", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Delegate execution"))
        .stdout(predicate::str::contains("--task"))
        .stdout(predicate::str::contains("--agent"))
        .stdout(predicate::str::contains("--execute"));
}

#[test]
fn agents_discover_cli_with_fake_runner() {
    let runner = fake_runner("qwen-discover", "#!/usr/bin/env bash\necho 'qwen 1.4.2'\n");
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");
    let adapters_file = state_dir.path().join("adapters.json");
    let models_file = state_dir.path().join("models.json");

    fs::write(
        &adapters_file,
        r#"{"schema_version":1,"adapters":[{"name":"qwen-code","binary":"qwen","status":"available","argv":["$MODEL","$TASK"]}]}"#,
    )
    .unwrap();

    fs::write(
        &models_file,
        r#"{"schema_version":2,"agents":{"qwen-code":[{"id":"qwen3.8-max","source":"runtime","confidence":"high","notes":"test model","cost_hint":0.002,"status":"active"}]}}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", &runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "agents",
            "discover",
            "--adapters-config",
            adapters_file.to_str().unwrap(),
            "--models-config",
            models_file.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"snapshot_at\""))
        .stdout(predicate::str::contains("\"source\": \"runtime_doctor\""))
        .stdout(predicate::str::contains("\"secrets_read\": false"))
        .stdout(predicate::str::contains("\"id\": \"qwen-code\""))
        .stdout(predicate::str::contains("\"version\": \"1.4.2\""))
        .stdout(predicate::str::contains("\"agents_persisted\": 1"));

    assert!(db.exists());
}

#[test]
fn agents_refresh_cli_with_fake_runner() {
    let runner = fake_runner("qwen-refresh", "#!/usr/bin/env bash\necho 'qwen 1.4.2'\n");
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");
    let adapters_file = state_dir.path().join("adapters.json");
    let models_file = state_dir.path().join("models.json");

    fs::write(
        &adapters_file,
        r#"{"schema_version":1,"adapters":[{"name":"qwen-code","binary":"qwen","status":"available","argv":["$MODEL","$TASK"]}]}"#,
    )
    .unwrap();

    fs::write(
        &models_file,
        r#"{"schema_version":2,"agents":{"qwen-code":[{"id":"qwen3.8-max","source":"runtime","confidence":"high","notes":"test model","cost_hint":0.002,"status":"active"}]}}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", &runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "agents",
            "refresh",
            "qwen-code",
            "--adapters-config",
            adapters_file.to_str().unwrap(),
            "--models-config",
            models_file.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"refreshed_at\""))
        .stdout(predicate::str::contains("\"source\": \"runtime_doctor\""))
        .stdout(predicate::str::contains("\"secrets_read\": false"))
        .stdout(predicate::str::contains("\"id\": \"qwen-code\""))
        .stdout(predicate::str::contains("\"version\": \"1.4.2\""))
        .stdout(predicate::str::contains("\"models_persisted\""));
}

#[test]
fn agents_doctor_cli_reports_health() {
    let fake_rtk = fake_runner("doctor-rtk", "#!/usr/bin/env bash\necho 'rtk 0.3.0'\n");
    let fake_vg = fake_runner("doctor-vg", "#!/usr/bin/env bash\necho 'vg 1.0.0'\n");
    let fake_engram = fake_runner("doctor-engram", "#!/usr/bin/env bash\necho 'engram 0.4.1'\n");

    let state_dir = tempfile::tempdir().unwrap();
    let adapters_file = state_dir.path().join("adapters.json");
    fs::write(
        &adapters_file,
        r#"{"schema_version":1,"adapters":[{"name":"qwen-code","binary":"qwen","status":"available","argv":["$MODEL","$TASK"]}]}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_RTK_BIN", &fake_rtk)
        .env("ORQ_VG_BIN", &fake_vg)
        .env("ORQ_ENGRAM_BIN", &fake_engram)
        .args([
            "agents",
            "doctor",
            "--adapters-config",
            adapters_file.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"doctor_at\""))
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"exit_code\": 0"))
        .stdout(predicate::str::contains("\"secrets_read\": false"))
        .stdout(predicate::str::contains("wrapper:rtk"))
        .stdout(predicate::str::contains("wrapper:vg"))
        .stdout(predicate::str::contains("wrapper:engram"));
}

#[test]
fn agents_doctor_cli_fails_on_missing_required_wrapper() {
    let state_dir = tempfile::tempdir().unwrap();
    let adapters_file = state_dir.path().join("adapters.json");
    fs::write(
        &adapters_file,
        r#"{"schema_version":1,"adapters":[{"name":"qwen-code","binary":"qwen","status":"available","argv":["$MODEL","$TASK"]}]}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_RTK_BIN", "/tmp/non-existent-rtk-bin-xyz-98765")
        .args([
            "agents",
            "doctor",
            "--adapters-config",
            adapters_file.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("\"status\": \"missing\""))
        .stdout(predicate::str::contains("\"exit_code\": 1"))
        .stdout(predicate::str::contains("\"secrets_read\": false"));
}

#[test]
fn models_snapshot_cli_exports_json() {
    let runner = fake_runner("qwen-snap", "#!/usr/bin/env bash\necho 'qwen 1.4.2'\n");
    let state_dir = tempfile::tempdir().unwrap();
    let snap_out = state_dir.path().join("snapshot-export.json");
    let adapters_file = state_dir.path().join("adapters.json");
    let models_file = state_dir.path().join("models.json");

    fs::write(
        &adapters_file,
        r#"{"schema_version":1,"adapters":[{"name":"qwen-code","binary":"qwen","status":"available","argv":["$MODEL","$TASK"]}]}"#,
    )
    .unwrap();

    fs::write(
        &models_file,
        r#"{"schema_version":2,"agents":{"qwen-code":[{"id":"qwen3.8-max","source":"runtime","confidence":"high","notes":"test model","cost_hint":0.002,"status":"active"}]}}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", &runner)
        .args([
            "models",
            "snapshot",
            "--output",
            snap_out.to_str().unwrap(),
            "--adapters-config",
            adapters_file.to_str().unwrap(),
            "--models-config",
            models_file.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"snapshot_id\""))
        .stdout(predicate::str::contains("\"fetched_at\""))
        .stdout(predicate::str::contains("\"source\": \"runtime_doctor\""))
        .stdout(predicate::str::contains("\"secrets_read\": false"))
        .stdout(predicate::str::contains("\"id\": \"qwen-code\""));

    assert!(snap_out.exists());
    let snap_content = fs::read_to_string(snap_out).unwrap();
    assert!(snap_content.contains("\"snapshot_id\""));
    assert!(snap_content.contains("\"secrets_read\": false"));
}

#[test]
fn orq_alias_supports_agents_and_models_snapshot() {
    let runner = fake_runner("qwen-alias", "#!/usr/bin/env bash\necho 'qwen 1.4.2'\n");
    let state_dir = tempfile::tempdir().unwrap();
    let db = state_dir.path().join("state.sqlite");
    let adapters_file = state_dir.path().join("adapters.json");
    let models_file = state_dir.path().join("models.json");

    fs::write(
        &adapters_file,
        r#"{"schema_version":1,"adapters":[{"name":"qwen-code","binary":"qwen","status":"available","argv":["$MODEL","$TASK"]}]}"#,
    )
    .unwrap();

    fs::write(
        &models_file,
        r#"{"schema_version":2,"agents":{"qwen-code":[{"id":"qwen3.8-max","source":"runtime","confidence":"high","notes":"test model","cost_hint":0.002,"status":"active"}]}}"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orq").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", &runner)
        .env("ORQ_STATE_DB", &db)
        .args([
            "agents",
            "discover",
            "--adapters-config",
            adapters_file.to_str().unwrap(),
            "--models-config",
            models_file.to_str().unwrap(),
            "--db-path",
            db.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"source\": \"runtime_doctor\""))
        .stdout(predicate::str::contains("\"secrets_read\": false"));
}



