use std::net::SocketAddr;
use std::time::Duration;

use freehand_relay::{
    AgentHeartbeat, AgentRole, AgentWorkStatus, RelayAgentClient, RelayAgentClientConfig,
    RelayServerConfig, RelayService, RelayServiceConfig, RelayStore,
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .ok_or("expected `init-store`, `serve`, or `agent-tunnel`")?;
    if args.next().is_some() {
        return Err("Relay command does not accept positional arguments".into());
    }
    if command == "init-store" {
        let runtime = freehand_relay::RelayRuntimeConfig::from_env()?;
        RelayStore::initialize(runtime.store_path)?;
        return Ok(());
    }
    if command == "agent-tunnel" {
        let client = RelayAgentClient::new(agent_tunnel_config_from_env()?)?;
        client.run().await?;
        return Ok(());
    }
    if command != "serve" {
        return Err("expected `init-store`, `serve`, or `agent-tunnel`".into());
    }

    let config = RelayServerConfig::from_env()?;
    let store = RelayStore::load(&config.runtime.store_path)?;
    let service = RelayService::new(
        store,
        RelayServiceConfig {
            presence_lease_seconds: config.runtime.presence_lease_seconds,
            secure_cookie: config.secure_cookie,
        },
    )?;
    let listener = TcpListener::bind(config.bind).await?;
    eprintln!("freehand relay listening on {}", config.bind);
    service.serve(listener).await?;
    Ok(())
}

fn agent_tunnel_config_from_env() -> Result<RelayAgentClientConfig, Box<dyn std::error::Error>> {
    let role = match required_env("FREEHAND_RELAY_AGENT_ROLE")?.as_str() {
        "master" => AgentRole::Master,
        "worker" => AgentRole::Worker,
        value => return Err(format!("invalid FREEHAND_RELAY_AGENT_ROLE `{value}`").into()),
    };
    let status = match required_env("FREEHAND_RELAY_AGENT_STATUS")?.as_str() {
        "idle" => AgentWorkStatus::Idle,
        "running" => AgentWorkStatus::Running,
        "waiting" => AgentWorkStatus::Waiting,
        "error" => AgentWorkStatus::Error,
        value => return Err(format!("invalid FREEHAND_RELAY_AGENT_STATUS `{value}`").into()),
    };
    let active_session_count =
        required_env("FREEHAND_RELAY_AGENT_ACTIVE_SESSION_COUNT")?.parse::<u32>()?;
    let local_daemon_addr =
        required_env("FREEHAND_RELAY_AGENT_LOCAL_ADDR")?.parse::<SocketAddr>()?;
    Ok(RelayAgentClientConfig {
        relay_base_url: required_env("FREEHAND_RELAY_AGENT_URL")?,
        access_token: required_env("FREEHAND_RELAY_AGENT_TOKEN")?,
        heartbeat: AgentHeartbeat {
            agent_id: required_env("FREEHAND_RELAY_AGENT_ID")?,
            display_name: required_env("FREEHAND_RELAY_AGENT_DISPLAY_NAME")?,
            node_id: required_env("FREEHAND_RELAY_AGENT_NODE_ID")?,
            role,
            status,
            active_session_count,
        },
        local_daemon_addr,
        local_adp_token: std::env::var("FREEHAND_RELAY_AGENT_LOCAL_ADP_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty()),
        heartbeat_interval: Duration::from_secs(15),
    })
}

fn required_env(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = std::env::var(name).map_err(|_| format!("missing required environment {name}"))?;
    if value.trim().is_empty() {
        return Err(format!("required environment {name} is empty").into());
    }
    Ok(value)
}
