use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_home_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time drift")
        .as_nanos();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "freehand-testkit-reason-smoke-home-{nanos}-{counter}"
    ))
}

#[test]
fn reason_smoke_bin_runs_usage_compaction() {
    let output = Command::new(env!("CARGO_BIN_EXE_freehand-reason-smoke"))
        .arg("reason-e2e")
        .arg("--agent")
        .arg("master")
        .arg("--scenario")
        .arg("usage-compaction")
        .output()
        .expect("run reason smoke");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("scenario=usage-compaction"));
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("rewrite_action=StageCompaction"));
    assert!(stdout.contains("rewrite_version=1"));
    assert!(stdout.contains("latest_usage_tokens=80"));
    assert!(stdout.contains("blocked=false"));
}

#[test]
fn reason_smoke_bin_runs_recovery_block() {
    let output = Command::new(env!("CARGO_BIN_EXE_freehand-reason-smoke"))
        .arg("reason-e2e")
        .arg("--agent")
        .arg("master")
        .arg("--scenario")
        .arg("recovery-block")
        .output()
        .expect("run reason smoke");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("scenario=recovery-block"));
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("rewrite_action=Block"));
    assert!(stdout.contains("rewrite_version=0"));
    assert!(stdout.contains("latest_usage_tokens=none"));
    assert!(stdout.contains("blocked=true"));
}

#[test]
fn reason_smoke_bin_runs_persist_smoke() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-reason-smoke"))
        .arg("reason-persist-smoke")
        .arg("--agent")
        .arg("master")
        .arg("--runtime-home")
        .arg(freehand_dir.as_os_str())
        .output()
        .expect("run reason smoke");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("restored_terminal=persisted smoke terminal"));
    assert!(stdout.contains("reason_seq=3"));
    assert!(stdout.contains("ui_sidecar_exists=true"));
    assert!(stdout.contains("session_index_entries=1"));

    fs::remove_dir_all(home).expect("cleanup");
}
