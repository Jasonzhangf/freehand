use std::future::pending;
use std::sync::{Arc, Mutex};

use freehand_server::{
    parse_bind_arg, render_webui_smoke, seed_webui_protocol_state, serve_webui_listener, usage,
};
use freehand_ui_protocol::StaticUiCommandDispatchPort;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    match run().await {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    }
}

async fn run() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage("freehand-server"));
    };
    match command.as_str() {
        "webui-smoke" if args.next().is_none() => Ok(render_webui_smoke()),
        "webui-serve-smoke" => {
            let bind_addr = parse_bind_arg(args)?;
            let listener = TcpListener::bind(bind_addr)
                .await
                .map_err(|err| format!("failed to bind {bind_addr}: {err}"))?;
            let local_addr = listener
                .local_addr()
                .map_err(|err| format!("failed to read local addr: {err}"))?;
            println!("freehand-server listening on http://{local_addr}");
            serve_webui_listener(
                listener,
                Arc::new(Mutex::new(seed_webui_protocol_state())),
                Arc::new(StaticUiCommandDispatchPort::default()),
                pending::<()>(),
            )
            .await
            .map_err(|err| format!("server error: {err}"))?;
            Ok(String::new())
        }
        _ => Err(usage("freehand-server")),
    }
}
