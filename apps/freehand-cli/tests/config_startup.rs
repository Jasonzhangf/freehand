use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
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

fn spawn_mock_server(
    status: u16,
    content_type: &'static str,
    response_body: String,
) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let mut raw = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buffer[..read]);
            if request_is_complete(&raw) {
                break;
            }
        }
        tx.send(String::from_utf8(raw).expect("utf8"))
            .expect("send");
        let response = format!(
            "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
            response_body.len()
        );
        stream.write_all(response.as_bytes()).expect("write");
    });
    (base_url, rx, handle)
}

fn tagged_completion_json(body: &str) -> String {
    format!("<freehand_completion>\n{body}\n</freehand_completion>")
}

fn complete_single_response(visible_text: &str) -> String {
    let tagged = tagged_completion_json(
        r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
    );
    format!(
        r#"{{"content":[{{"type":"text","text":"{visible}\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":82}},"stop_reason":"end_turn"}}"#,
        visible = visible_text,
        tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
    )
}

fn complete_stream_response(visible_text: &str) -> String {
    let tagged = tagged_completion_json(
        r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
    );
    format!(
        concat!(
            "event: content_block_start\n",
            "data: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"thinking\",\"thinking\":\"\"}}}}\n\n",
            "event: content_block_delta\n",
            "data: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"thinking_delta\",\"thinking\":\"thinking\"}}}}\n\n",
            "event: content_block_stop\n",
            "data: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n",
            "event: content_block_start\n",
            "data: {{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n",
            "event: content_block_delta\n",
            "data: {{\"type\":\"content_block_delta\",\"index\":1,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{text}\"}}}}\n\n",
            "event: content_block_stop\n",
            "data: {{\"type\":\"content_block_stop\",\"index\":1}}\n\n",
            "event: message_delta\n",
            "data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"input_tokens\":14,\"output_tokens\":82}}}}\n\n",
            "event: message_stop\n",
            "data: {{\"type\":\"message_stop\"}}\n\n"
        ),
        text = format!("{visible_text}\\n{tagged}")
            .replace('\n', "\\n")
            .replace('"', "\\\"")
    )
}

fn request_is_complete(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(raw);
    let Some(header_end) = text.find("\r\n\r\n") else {
        return false;
    };
    let content_length = text[..header_end]
        .lines()
        .find_map(|line| {
            line.strip_prefix("content-length: ")
                .or_else(|| line.strip_prefix("Content-Length: "))
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    raw.len() >= header_end + 4 + content_length
}

#[test]
fn cli_selects_named_agent_from_default_config_path() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "chat_completions"
baseURL = "http://guizhouyun.site:2080"
defaultModel = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
apiKey = "sk-inline"

[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"
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
    assert!(stdout.contains("provider=mini27"));
    assert!(stdout.contains("provider_type=openai"));
    assert!(stdout.contains("provider_protocol=chat_completions"));
    assert!(stdout.contains("default_model=MiniMax-M2.7"));
    assert!(stdout.contains("base_url=http://guizhouyun.site:2080"));
    assert!(stdout.contains("provider_auth=apikey"));
    assert!(stdout.contains("restart_required_on_change=true"));
    assert!(!stdout.contains("sk-inline"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_e2e_usage_compaction_smoke() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .arg("reason-e2e")
        .arg("--agent")
        .arg("master")
        .arg("--scenario")
        .arg("usage-compaction")
        .output()
        .expect("run cli");

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

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_e2e_recovery_block_smoke() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .arg("reason-e2e")
        .arg("--agent")
        .arg("master")
        .arg("--scenario")
        .arg("recovery-block")
        .output()
        .expect("run cli");

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

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_live_single_shot_mock() {
    let (base_url, rx, handle) =
        spawn_mock_server(200, "application/json", complete_single_response("pong"));
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        format!(
            r#"
[providers.minimonth]
id = "minimonth"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "{base_url}"
default_model = "MiniMax-M2.7"

[providers.minimonth.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "minimonth"
"#
        ),
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .arg("reason-live")
        .arg("--agent")
        .arg("master")
        .arg("--prompt")
        .arg("reply exactly pong")
        .output()
        .expect("run cli");

    let raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("provider=minimonth"));
    assert!(stdout.contains("stream=false"));
    assert!(stdout.contains("text=pong"));
    assert!(stdout.contains("usage_input_tokens=14"));
    assert!(stdout.contains("usage_output_tokens=82"));
    assert!(stdout.contains("rounds=1"));
    assert!(stdout.contains("schema_rejections=0"));
    assert!(stdout.contains("terminal=Summary: pong"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_live_stream_mock() {
    let (base_url, rx, handle) =
        spawn_mock_server(200, "text/event-stream", complete_stream_response("pong"));
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        format!(
            r#"
[providers.minimonth]
id = "minimonth"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "{base_url}"
default_model = "MiniMax-M2.7"

[providers.minimonth.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "minimonth"
"#
        ),
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .arg("reason-live")
        .arg("--agent")
        .arg("master")
        .arg("--prompt")
        .arg("reply exactly pong")
        .arg("--stream")
        .output()
        .expect("run cli");

    let raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
    assert!(stdout.contains("stream=true"));
    assert!(stdout.contains("text=pong"));
    assert!(stdout.contains("reasoning_events="));
    assert!(stdout.contains("usage_input_tokens=14"));
    assert!(stdout.contains("rounds=1"));
    assert!(stdout.contains("schema_rejections=0"));
    assert!(stdout.contains("terminal=Summary: pong"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_live_unsupported_provider_smoke() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "chat_completions"
base_url = "http://127.0.0.1:1"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .arg("reason-live")
        .arg("--agent")
        .arg("master")
        .arg("--prompt")
        .arg("reply exactly pong")
        .output()
        .expect("run cli");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(stderr.contains("is not supported"));

    fs::remove_dir_all(home).expect("cleanup");
}
