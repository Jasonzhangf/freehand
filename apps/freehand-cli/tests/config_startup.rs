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
    std::env::temp_dir().join(format!("freehand-cli-home-{nanos}-{counter}"))
}

#[test]
fn cli_selects_named_agent_from_default_config_path() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_CLI_TOKEN"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .arg("--agent")
        .arg("master")
        .output()
        .expect("run cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("mode=master"));
    assert!(stdout.contains("pair_token_env=FREEHAND_CLI_TOKEN"));
    assert!(stdout.contains("restart_required_on_change=true"));

    fs::remove_dir_all(home).expect("cleanup");
}
