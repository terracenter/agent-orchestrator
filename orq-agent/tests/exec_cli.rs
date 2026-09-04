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
        .stdout(predicate::str::contains("\"schema_version\": 2"))
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
