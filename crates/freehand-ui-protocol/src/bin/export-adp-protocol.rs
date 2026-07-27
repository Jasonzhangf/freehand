use std::env;
use std::fs;
use std::path::PathBuf;

use freehand_ui_protocol::{adp_protocol_manifest_json, adp_protocol_webui_module};

enum ExportMode {
    Json,
    Js,
}

fn write_output(mode: ExportMode, output_path: PathBuf) -> Result<(), String> {
    let body = match mode {
        ExportMode::Json => adp_protocol_manifest_json(),
        ExportMode::Js => adp_protocol_webui_module(),
    };
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent {}: {err}", parent.display()))?;
    }
    fs::write(&output_path, body)
        .map_err(|err| format!("write {}: {err}", output_path.display()))?;
    Ok(())
}

fn main() {
    let mut args = env::args().skip(1);
    let result = match (args.next().as_deref(), args.next().as_deref(), args.next()) {
        (Some("--json"), Some(path), None) => write_output(ExportMode::Json, PathBuf::from(path)),
        (Some("--js"), Some(path), None) => write_output(ExportMode::Js, PathBuf::from(path)),
        _ => Err(
            "usage: cargo run -p freehand-ui-protocol --bin export-adp-protocol -- <--json|--js> <path>"
                .to_owned(),
        ),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
