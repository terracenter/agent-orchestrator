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
fn exec_qwen_fake_succeeds_with_receipt() {
    let runner = fake_runner("qwen-ok", "#!/usr/bin/env bash\necho fake-qwen-ok\n");
    let task = std::env::temp_dir().join(format!("orq-agent-task-{}.md", std::process::id()));
    fs::write(&task, "hello fake").unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
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
fn smoke_qwen_fake_succeeds_with_receipt() {
    let runner = fake_runner(
        "qwen-smoke",
        "#!/usr/bin/env bash\necho 'ORQ_SMOKE_OK agent=qwen-code model=qwen3.8-max'\n",
    );

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .args([
            "smoke",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--timeout",
            "5",
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

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.args([
        "exec",
        "--agent",
        "missing-agent",
        "--model",
        "missing-model",
        "--task-file",
        task.to_str().unwrap(),
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

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.args([
        "exec",
        "--agent",
        "qwen-code",
        "--model",
        "qwen3.8-max",
        "--task-file",
        task.to_str().unwrap(),
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

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env(
        "ORQ_AGENT_BIN_QWEN_CODE",
        "/tmp/orq-agent-definitely-missing-binary",
    )
    .args([
        "exec",
        "--agent",
        "qwen-code",
        "--model",
        "qwen3.8-max",
        "--task-file",
        task.to_str().unwrap(),
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

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
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

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
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

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--timeout",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"timed_out\""));

    std::thread::sleep(std::time::Duration::from_secs(3));
    assert!(!marker.exists(), "timeout left an orphaned process behind");
}

#[test]
fn exec_qwen_fake_timeout_is_reported() {
    let runner = fake_runner("qwen-sleep", "#!/usr/bin/env bash\nsleep 5\n");
    let task =
        std::env::temp_dir().join(format!("orq-agent-timeout-task-{}.md", std::process::id()));
    fs::write(&task, "hello timeout").unwrap();

    let mut cmd = Command::cargo_bin("orq-agent").unwrap();
    cmd.env("ORQ_AGENT_BIN_QWEN_CODE", runner)
        .args([
            "exec",
            "--agent",
            "qwen-code",
            "--model",
            "qwen3.8-max",
            "--task-file",
            task.to_str().unwrap(),
            "--timeout",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"timed_out\""));
}
