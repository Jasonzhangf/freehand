use super::*;
use sha2::{Digest, Sha256};

const OPENMINIS_UI_EVIDENCE_ROOT: &str = "docs/migrations/openminis-ui/evidence/";
const OPENMINIS_UI_MANIFEST_PATH: &str = "docs/migrations/openminis-ui/ui-tree.manifest.json";

fn evidence_gate_contract(
    gate_id: &str,
    node_id: &str,
) -> Result<(String, &'static str, &'static str, &'static [&'static str]), String> {
    match gate_id {
        "openminis_ui_migration_manifest" | "cargo_run_xtask_gates_check" => Ok((
            format!("cargo run -p xtask -- openminis-ui verify-node {node_id}"),
            "node_repository_gate",
            "xtask.openminis-ui-node",
            &["node_source_gates_passed"],
        )),
        "webui_online_e2e" => Ok((
            "make verify-webui-online".to_owned(),
            "webui_online",
            "freehand.webui_online",
            &[
                "daemon_hosted",
                "owner_truth_verified",
                "dom_assertions_passed",
            ],
        )),
        "android_device_e2e" => Ok((
            "apps/freehand-android/scripts/verify-device-ui.sh".to_owned(),
            "android_device",
            "freehand.android_device_ui",
            &["apk_installed", "device_interaction_passed", "logcat_clean"],
        )),
        "openminis_ui_legacy_online_no_touch" => Ok((
            "make verify-webui-online".to_owned(),
            "legacy_online_no_touch",
            "freehand.webui_online.legacy_no_touch",
            &["daemon_hosted", "legacy_not_loaded", "owner_truth_verified"],
        )),
        _ => Err(format!(
            "unregistered OpenMinis UI evidence gate `{gate_id}`"
        )),
    }
}

pub(super) fn verify_openminis_ui_evidence(
    root: &Path,
    node_id: &str,
    node: &serde_json::Map<String, Value>,
    verification_gates: &BTreeSet<String>,
) -> Result<(), String> {
    let evidence = node
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("migration node `{node_id}` evidence must be an array"))?;
    let mut evidenced_gates = BTreeSet::new();
    let mut evidence_paths = BTreeSet::new();
    let mut repository_revisions = BTreeSet::new();
    for record in evidence {
        let record = record
            .as_object()
            .ok_or_else(|| format!("migration node `{node_id}` evidence must contain objects"))?;
        let evidence_node_id = required_string(
            record,
            "node_id",
            &format!("migration node {node_id} evidence"),
        )?;
        if evidence_node_id != node_id {
            return Err(format!(
                "migration node `{node_id}` evidence node_id drift: `{evidence_node_id}`"
            ));
        }
        let gate_id = required_string(
            record,
            "gate_id",
            &format!("migration node {node_id} evidence"),
        )?;
        let command = required_string(
            record,
            "command",
            &format!("migration node {node_id} evidence"),
        )?;
        let artifact_path = required_string(
            record,
            "artifact_path",
            &format!("migration node {node_id} evidence"),
        )?;
        verify_openminis_ui_evidence_path(artifact_path, "artifact_path")?;
        let result = required_string(
            record,
            "result",
            &format!("migration node {node_id} evidence"),
        )?;
        let online_run_id = required_string(
            record,
            "online_run_id",
            &format!("migration node {node_id} evidence"),
        )?;
        let (expected_command, proof_kind, verifier_id, required_assertions) =
            evidence_gate_contract(gate_id, node_id)?;
        if command != expected_command || online_run_id.trim().is_empty() || result != "passed" {
            return Err(format!(
                "migration node `{node_id}` evidence for `{gate_id}` is not a passed executable online proof"
            ));
        }
        if !evidenced_gates.insert(gate_id.to_owned()) {
            return Err(format!(
                "migration node `{node_id}` evidence has duplicate gate `{gate_id}`"
            ));
        }
        let artifact = read_openminis_ui_evidence_artifact(root, node_id, gate_id, artifact_path)?;
        evidence_paths.insert(artifact_path.to_owned());
        for (field, expected) in [
            ("node_id", evidence_node_id),
            ("gate_id", gate_id),
            ("command", command),
            ("result", result),
            ("online_run_id", online_run_id),
        ] {
            let actual = artifact.get(field).and_then(Value::as_str).ok_or_else(|| {
                format!(
                    "migration node `{node_id}` evidence artifact `{artifact_path}` missing string field `{field}`"
                )
            })?;
            if actual != expected {
                return Err(format!(
                    "migration node `{node_id}` evidence artifact `{artifact_path}` field `{field}` drift: expected `{expected}`, got `{actual}`"
                ));
            }
        }
        if artifact.get("proof_kind").and_then(Value::as_str) != Some(proof_kind) {
            return Err(format!(
                "migration node `{node_id}` evidence artifact `{artifact_path}` has wrong proof_kind for `{gate_id}`"
            ));
        }
        let report_path = artifact
            .get("verifier_report_path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "migration node `{node_id}` evidence artifact `{artifact_path}` lacks verifier_report_path"
                )
            })?;
        if report_path == artifact_path {
            return Err(format!(
                "migration node `{node_id}` evidence artifact must bind a distinct verifier report"
            ));
        }
        verify_openminis_ui_evidence_path(report_path, "verifier_report_path")?;
        let report_file = existing_repository_path(
            root,
            report_path,
            &format!("migration node `{node_id}` evidence verifier report"),
        )?;
        if !report_file.is_file() {
            return Err(format!(
                "migration node `{node_id}` verifier report is not a file: `{report_path}`"
            ));
        }
        evidence_paths.insert(report_path.to_owned());
        let report_bytes = fs::read(&report_file)
            .map_err(|err| format!("read verifier report `{report_path}`: {err}"))?;
        let report_sha256 = format!("{:x}", Sha256::digest(&report_bytes));
        if artifact
            .get("verifier_report_sha256")
            .and_then(Value::as_str)
            != Some(&report_sha256)
        {
            return Err(format!(
                "migration node `{node_id}` verifier report digest drift for `{report_path}`"
            ));
        }
        let report: Value = serde_json::from_slice(&report_bytes)
            .map_err(|err| format!("parse verifier report `{report_path}`: {err}"))?;
        for (field, expected) in [
            ("schema_version", "freehand.verifier-report.v1"),
            ("verifier_id", verifier_id),
            ("node_id", node_id),
            ("migration_unit_id", &format!("ui_migration.{node_id}")),
            ("command", command),
            ("online_run_id", online_run_id),
            ("result", "passed"),
        ] {
            if report.get(field).and_then(Value::as_str) != Some(expected) {
                return Err(format!(
                    "migration node `{node_id}` verifier report `{report_path}` field `{field}` drift"
                ));
            }
        }
        let report_commit = report
            .get("repository_commit")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                format!(
                    "migration node `{node_id}` verifier report `{report_path}` lacks repository_commit"
                )
            })?;
        if report_commit.len() != 40 || !report_commit.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(format!(
                "migration node `{node_id}` verifier report `{report_path}` repository_commit must be a full 40-character git SHA"
            ));
        }
        let report_tree = report
            .get("repository_tree")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                format!(
                    "migration node `{node_id}` verifier report `{report_path}` lacks repository_tree"
                )
            })?;
        verify_openminis_ui_report_revision(root, report_commit, report_tree)?;
        provenance::verify_online_report_provenance(gate_id, &report)?;
        repository_revisions.insert((report_commit.to_owned(), report_tree.to_owned()));
        if report.get("exit_code").and_then(Value::as_i64) != Some(0) {
            return Err(format!(
                "migration node `{node_id}` verifier report `{report_path}` is not a successful process result"
            ));
        }
        let started = report
            .get("started_at_unix_ms")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                format!("migration node `{node_id}` verifier report lacks start time")
            })?;
        let finished = report
            .get("finished_at_unix_ms")
            .and_then(Value::as_u64)
            .filter(|value| *value >= started)
            .ok_or_else(|| {
                format!("migration node `{node_id}` verifier report has invalid finish time")
            })?;
        if finished == 0 {
            return Err(format!(
                "migration node `{node_id}` verifier report has invalid finish time"
            ));
        }
        let assertions = report
            .get("assertions")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                format!(
                    "migration node `{node_id}` verifier report `{report_path}` lacks assertions"
                )
            })?;
        for assertion in required_assertions {
            if assertions.get(*assertion).and_then(Value::as_bool) != Some(true) {
                return Err(format!(
                    "migration node `{node_id}` evidence `{gate_id}` lacks passed assertion `{assertion}`"
                ));
            }
        }
    }
    if &evidenced_gates != verification_gates {
        return Err(format!(
            "migration node `{node_id}` evidence gate coverage drift: required={verification_gates:?}, evidenced={evidenced_gates:?}"
        ));
    }
    if repository_revisions.len() != 1 {
        return Err(format!(
            "migration node `{node_id}` evidence reports must attest one source revision, found {repository_revisions:?}"
        ));
    }
    let (repository_commit, _) = repository_revisions
        .first()
        .ok_or_else(|| format!("migration node `{node_id}` has no attested source revision"))?;
    verify_openminis_ui_evidence_worktree(root, repository_commit, &evidence_paths)?;
    Ok(())
}

fn verify_openminis_ui_evidence_path(path: &str, field: &str) -> Result<(), String> {
    repository_relative_path(path, &format!("OpenMinis UI evidence {field}"))?;
    if !path.starts_with(OPENMINIS_UI_EVIDENCE_ROOT)
        || path.len() == OPENMINIS_UI_EVIDENCE_ROOT.len()
    {
        return Err(format!(
            "OpenMinis UI evidence {field} must be under `{OPENMINIS_UI_EVIDENCE_ROOT}`"
        ));
    }
    Ok(())
}

fn verify_openminis_ui_report_revision(
    root: &Path,
    repository_commit: &str,
    repository_tree: &str,
) -> Result<(), String> {
    let object_type = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "-t", repository_commit])
        .output()
        .map_err(|err| format!("inspect attested repository revision: {err}"))?;
    if !object_type.status.success()
        || String::from_utf8_lossy(&object_type.stdout).trim() != "commit"
    {
        return Err(format!(
            "OpenMinis UI verifier report repository_commit `{repository_commit}` must resolve to a commit object"
        ));
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", &format!("{repository_commit}^{{tree}}")])
        .output()
        .map_err(|err| format!("resolve attested repository commit: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "OpenMinis UI verifier report repository commit `{repository_commit}` does not resolve: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let actual_tree = String::from_utf8(output.stdout)
        .map_err(|err| format!("git tree output was not UTF-8: {err}"))?
        .trim()
        .to_owned();
    if actual_tree != repository_tree {
        return Err(format!(
            "OpenMinis UI verifier report repository tree drift: commit `{repository_commit}` has `{actual_tree}`, report has `{repository_tree}`"
        ));
    }
    Ok(())
}

fn verify_openminis_ui_evidence_worktree(
    root: &Path,
    repository_commit: &str,
    evidence_paths: &BTreeSet<String>,
) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            repository_commit,
            "--",
        ])
        .output()
        .map_err(|err| format!("compare attested repository source: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git -C {} diff against attested commit `{repository_commit}` failed: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    verify_openminis_ui_changed_paths(&output.stdout, evidence_paths)?;

    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "show",
            &format!("{repository_commit}:{OPENMINIS_UI_MANIFEST_PATH}"),
        ])
        .output()
        .map_err(|err| format!("read attested OpenMinis UI manifest: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "OpenMinis UI verifier report cannot read attested manifest `{repository_commit}:{OPENMINIS_UI_MANIFEST_PATH}`: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut attested_manifest: Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("parse attested OpenMinis UI manifest: {err}"))?;
    let current_raw = fs::read(root.join(OPENMINIS_UI_MANIFEST_PATH))
        .map_err(|err| format!("read current OpenMinis UI manifest: {err}"))?;
    let mut current_manifest: Value = serde_json::from_slice(&current_raw)
        .map_err(|err| format!("parse current OpenMinis UI manifest: {err}"))?;
    for manifest in [&mut attested_manifest, &mut current_manifest] {
        let object = manifest
            .as_object_mut()
            .ok_or_else(|| "OpenMinis UI manifest must be an object".to_owned())?;
        object.remove("status");
        let nodes = object
            .get_mut("nodes")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "OpenMinis UI manifest must contain a nodes array".to_owned())?;
        for node in nodes {
            let node = node
                .as_object_mut()
                .ok_or_else(|| "OpenMinis UI manifest nodes must be objects".to_owned())?;
            node.remove("status");
            node.remove("evidence");
            node.remove("legacy_retirement");
        }
    }
    if current_manifest != attested_manifest {
        return Err(
            "OpenMinis UI verifier report cannot attest manifest non-lifecycle contract drift"
                .to_owned(),
        );
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .output()
        .map_err(|err| format!("list untracked repository paths: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git -C {} ls-files failed: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    verify_openminis_ui_changed_paths(&output.stdout, evidence_paths)
}

fn verify_openminis_ui_changed_paths(
    paths: &[u8],
    evidence_paths: &BTreeSet<String>,
) -> Result<(), String> {
    for record in paths
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let path = std::str::from_utf8(record)
            .map_err(|err| format!("git changed path was not UTF-8: {err}"))?;
        if path != OPENMINIS_UI_MANIFEST_PATH && !evidence_paths.contains(path) {
            return Err(format!(
                "OpenMinis UI verifier report cannot attest dirty repository path `{path}`"
            ));
        }
    }
    Ok(())
}

pub(super) fn read_openminis_ui_evidence_artifact(
    root: &Path,
    node_id: &str,
    gate_id: &str,
    artifact_path: &str,
) -> Result<Value, String> {
    let artifact = existing_repository_path(
        root,
        artifact_path,
        &format!("migration node `{node_id}` evidence for `{gate_id}` artifact_path"),
    )?;
    if !artifact.is_file() {
        return Err(format!(
            "migration node `{node_id}` evidence for `{gate_id}` references non-file artifact `{artifact_path}`"
        ));
    }
    let raw = fs::read_to_string(&artifact).map_err(|err| {
        format!("read migration node `{node_id}` evidence artifact `{artifact_path}`: {err}")
    })?;
    serde_json::from_str(&raw).map_err(|err| {
        format!(
            "migration node `{node_id}` evidence artifact `{artifact_path}` is not valid JSON: {err}"
        )
    })
}

pub(crate) mod provenance;
mod retirement;

pub(super) fn verify_openminis_ui_legacy_retirement(
    root: &Path,
    node_id: &str,
    node: &serde_json::Map<String, Value>,
    verification_gates: &BTreeSet<String>,
) -> Result<(), String> {
    retirement::check_openminis_ui_legacy_retirement(root, node_id, node, verification_gates)
}
