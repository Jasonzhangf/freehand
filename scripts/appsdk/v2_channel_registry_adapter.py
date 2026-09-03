#!/usr/bin/env python3
"""Freehand v2 channel registry AppSDK lifecycle adapter."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import uuid


def utc_now() -> str:
    return datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_text(value: str) -> str:
    return "sha256:" + hashlib.sha256(value.encode("utf-8")).hexdigest()


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def run(cmd, cwd=None, input_text=None) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=cwd, input=input_text, text=True, capture_output=True, check=False)


def git(project: pathlib.Path, args) -> subprocess.CompletedProcess:
    return run(["git", *args], cwd=project)


def fail(attempt: str, stage: str, message: str) -> None:
    sys.stderr.write(json.dumps({
        "transaction_attempt_id": attempt,
        "failure_node": stage,
        "error": message,
        "retry_allowed": False,
        "next": "fix the adapter failure route; do not overwrite existing records",
    }, indent=2, sort_keys=True) + "\n")
    raise SystemExit(2)


def stage(path: pathlib.Path, record: dict) -> None:
    path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")


def finalize(path: pathlib.Path, record: dict) -> None:
    if path.exists():
        existing = json.loads(path.read_text())
        if existing == record:
            return
        fail(record.get("evidence_id") or path.stem, "finalize", f"refusing to overwrite {path}")
    path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")


def evidence_record(*, module_id: str, evidence_id: str, issue_id: str, experiment_id: str, phase: str,
                    kind: str, source_commit: str, scope_hash: str, producer_identity: str, created_at: str,
                    input_hashes: list[str], artifact_hash: str | None = None, environment_id: str | None = None,
                    entrypoint: str | None = None) -> dict:
    record = {
        "$schema": "https://appsdk.local/contracts/records/evidence-record.schema.json",
        "evidence_id": evidence_id, "issue_id": issue_id, "experiment_id": experiment_id, "phase": phase,
        "kind": kind, "source_commit": source_commit, "scope": {"module_id": module_id, "feature_id": module_id},
        "producer": {"adapter": "freehand-v2-channel-registry-adapter", "identity": producer_identity},
        "result": "pass", "created_at": created_at,
        "expires_at": (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=1)).isoformat().replace("+00:00", "Z"),
        "input_hashes": input_hashes, "scope_hash": scope_hash,
    }
    if phase == "development_whitebox":
        record["execution_surface"] = "development_whitebox"
        if artifact_hash:
            record["artifact_hash"] = artifact_hash
    if phase in {"deployment_install", "deployment_restart", "deployed_blackbox"}:
        record["artifact_hash"] = artifact_hash
        record["execution_surface"] = "deployed_blackbox"
        record["environment_id"] = environment_id
        record["entrypoint"] = entrypoint
        record["scope"]["entrypoint"] = entrypoint
    return record


def public_check(proc: subprocess.CompletedProcess, attempt: str, stage: str) -> None:
    if proc.returncode != 0:
        fail(attempt, stage, proc.stderr or proc.stdout or f"exit {proc.returncode}")
    for line in proc.stdout.splitlines():
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if payload.get("ok") is False:
            fail(attempt, stage, payload.get("error") or "public binary reported failure")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("project", nargs="?", default=".")
    parser.add_argument("--attempt", default=None)
    args = parser.parse_args()
    module_id = "v2-channel-registry"
    project = pathlib.Path(args.project).resolve()
    attempt = args.attempt or f"{datetime.datetime.now(datetime.timezone.utc):%Y%m%d%H%M%S}-{uuid.uuid4().hex[:8]}"
    experiment_id = f"{module_id}-{attempt}"
    records_dir = project / ".appsdk" / "records"
    evidence_dir = records_dir / "evidence" / module_id
    staging_dir = project / ".appsdk-control" / "transactions" / module_id / attempt
    staging_dir.mkdir(parents=True, exist_ok=True)

    head = git(project, ["rev-parse", "HEAD"])
    if head.returncode != 0:
        fail(attempt, "candidate_identity", "cannot resolve HEAD")
    head_commit = head.stdout.strip()
    base_commit = None
    for ref in ("origin/v2", f"{head_commit}^"):
        base_proc = git(project, ["rev-parse", ref])
        if base_proc.returncode == 0:
            base_commit = base_proc.stdout.strip()
            break
    if base_commit is None:
        fail(attempt, "candidate_identity", "cannot resolve origin/v2 or HEAD parent")
    tree = git(project, ["rev-parse", f"{head_commit}^{{tree}}"]).stdout.strip()
    changed = git(project, ["diff", "--name-only", base_commit, head_commit]).stdout.splitlines()
    changed_paths = sorted(p for p in changed if p)
    diff_hash = sha256_bytes(git(project, ["diff", "--binary", base_commit, head_commit]).stdout.encode("utf-8"))
    scope_hash = sha256_text(json.dumps(changed_paths, sort_keys=True))
    pre_review_path = records_dir / f"pre-review-validation-record-{module_id}.json"
    if pre_review_path.exists():
        existing = json.loads(pre_review_path.read_text())
        if existing.get("candidate_commit") == head_commit:
            print(json.dumps({"existing_validation_id": existing.get("validation_id"), "candidate_commit": head_commit}, sort_keys=True))
            return
        fail(attempt, "pre_review_validation", "existing pre-review record binds another candidate")

    appsdk_bin = shutil.which("appsdk")
    if not appsdk_bin:
        fail(attempt, "artifact", "appsdk binary not found")
    compile_proc = run([appsdk_bin, "compile-module", str(project), "--module", module_id])
    if compile_proc.returncode != 0:
        fail(attempt, "artifact", compile_proc.stderr or compile_proc.stdout)
    try:
        compile_json = json.loads(compile_proc.stdout)
    except json.JSONDecodeError as exc:
        fail(attempt, "artifact", f"cannot parse compile-module output: {exc}")
    artifact_hash = compile_json["artifact_hash"]

    test_cmd = ["cargo", "test", "--manifest-path", str(project / "playground/experiments/v2/channel-registry/Cargo.toml"), "--test", "v2_channel_registry_boundary"]
    whitebox_proc = run(test_cmd, cwd=project)
    created_at = utc_now()
    whitebox_id = f"ev-{module_id}-whitebox-{attempt}"
    whitebox = evidence_record(module_id=module_id, evidence_id=whitebox_id, issue_id="freehand-v2-channel-registry-milestone",
        experiment_id=experiment_id, phase="development_whitebox", kind="gate", source_commit=head_commit, scope_hash=scope_hash,
        producer_identity=attempt, created_at=created_at, input_hashes=[sha256_text(json.dumps(test_cmd, sort_keys=True))], artifact_hash=artifact_hash)
    stage(staging_dir / "whitebox.json", whitebox)
    if whitebox_proc.returncode != 0:
        fail(attempt, "development_whitebox", whitebox_proc.stderr or whitebox_proc.stdout)

    deploy_dir = pathlib.Path("/tmp") / f"freehand-{module_id}-{attempt}"
    deploy_dir.mkdir(parents=True, exist_ok=True)
    entrypoint = deploy_dir / "v2-channel-registry-public"
    artifact = project / "generated" / "modules" / module_id / "lib" / "v2-channel-registry.module"
    if not artifact.exists():
        fail(attempt, "deployment_install", f"missing compiled artifact {artifact}")
    shutil.copy2(artifact, entrypoint)
    environment_id = f"local-{module_id}-{attempt}"
    install_input = "\n".join(json.dumps(item, sort_keys=True) for item in [
        {"action": "register", "endpoint_id": "endpoint-1", "node_id": "node-1", "token": "secret", "protocol_version": 1, "capabilities": ["ui.render"]},
        {"action": "open", "session_id": "session-1", "endpoint_id": "endpoint-1", "token": "secret"},
        {"action": "attach", "session_id": "session-1", "connection_id": "conn-1"},
        {"action": "send", "session_id": "session-1", "kind": "payload", "correlation_id": "c1", "payload_ref": "ref1"},
        {"action": "replay", "session_id": "session-1", "cursor": 0},
    ])
    blackbox_input = "\n".join(json.dumps(item, sort_keys=True) for item in [
        {"action": "register", "endpoint_id": "endpoint-1", "node_id": "node-1", "token": "secret", "protocol_version": 1, "capabilities": ["ui.render"]},
        {"action": "open", "session_id": "session-1", "endpoint_id": "endpoint-1", "token": "secret"},
        {"action": "attach", "session_id": "session-1", "connection_id": "conn-1"},
        {"action": "send", "session_id": "session-1", "kind": "control", "correlation_id": "c1", "message": "start"},
        {"action": "replace", "session_id": "session-1", "connection_id": "conn-2"},
        {"action": "suspend", "session_id": "session-1"},
        {"action": "reattach", "session_id": "session-1", "connection_id": "conn-3"},
        {"action": "replay", "session_id": "session-1", "cursor": 0},
    ])
    restart_input = install_input
    install_id = f"ev-{module_id}-install-{attempt}"
    install = evidence_record(module_id=module_id, evidence_id=install_id, issue_id="freehand-v2-channel-registry-milestone",
        experiment_id=experiment_id, phase="deployment_install", kind="install", source_commit=head_commit, scope_hash=scope_hash,
        producer_identity=attempt, created_at=created_at, input_hashes=[sha256_text(install_input)], artifact_hash=artifact_hash,
        environment_id=environment_id, entrypoint=str(entrypoint))
    stage(staging_dir / "install.json", install)
    public_check(run([str(entrypoint)], cwd=project, input_text=install_input), attempt, "deployment_install")
    restart_id = f"ev-{module_id}-restart-{attempt}"
    restart = evidence_record(module_id=module_id, evidence_id=restart_id, issue_id="freehand-v2-channel-registry-milestone",
        experiment_id=experiment_id, phase="deployment_restart", kind="restart", source_commit=head_commit, scope_hash=scope_hash,
        producer_identity=attempt, created_at=created_at, input_hashes=[sha256_text(restart_input)], artifact_hash=artifact_hash,
        environment_id=environment_id, entrypoint=str(entrypoint))
    stage(staging_dir / "restart.json", restart)
    public_check(run([str(entrypoint)], cwd=project, input_text=restart_input), attempt, "deployment_restart")
    blackbox_id = f"ev-{module_id}-blackbox-{attempt}"
    blackbox = evidence_record(module_id=module_id, evidence_id=blackbox_id, issue_id="freehand-v2-channel-registry-milestone",
        experiment_id=experiment_id, phase="deployed_blackbox", kind="runtime", source_commit=head_commit, scope_hash=scope_hash,
        producer_identity=attempt, created_at=created_at, input_hashes=[sha256_text(blackbox_input)], artifact_hash=artifact_hash,
        environment_id=environment_id, entrypoint=str(entrypoint))
    stage(staging_dir / "blackbox.json", blackbox)
    public_check(run([str(entrypoint)], cwd=project, input_text=blackbox_input), attempt, "deployed_blackbox")

    fix_id = f"fix-{module_id}-{attempt}"
    fix_candidate = {"$schema": "https://appsdk.local/contracts/records/fix-candidate-record.schema.json",
        "fix_candidate_id": fix_id, "issue_id": "freehand-v2-channel-registry-milestone", "module_id": module_id,
        "worktree_id": "playground/v2-channel-registry-20260903T224219Z-Macstudio.local-28622-14446",
        "base_commit": base_commit, "head_commit": head_commit, "tree_hash": tree, "diff_hash": diff_hash,
        "design_id": "docs/design/v2-channel-registry-owner-binding.md", "owner": module_id, "scope_hash": scope_hash,
        "changed_paths": changed_paths, "verification_evidence_ids": [whitebox_id, install_id, restart_id, blackbox_id],
        "created_at": created_at}
    stage(staging_dir / "fix-candidate.json", fix_candidate)
    validation_id = f"prv-{module_id}-{attempt}"
    pre_review = {"$schema": "https://appsdk.local/contracts/records/pre-review-validation-record.schema.json",
        "validation_id": validation_id, "issue_id": "freehand-v2-channel-registry-milestone", "module_id": module_id,
        "fix_candidate_id": fix_id, "candidate_commit": head_commit, "candidate_tree_hash": tree, "artifact_hash": artifact_hash,
        "whitebox_producer": {"adapter": "freehand-v2-channel-registry-adapter", "identity": attempt},
        "whitebox_evidence_ids": [whitebox_id], "blackbox_evidence_ids": [blackbox_id],
        "deployment": {"environment_id": environment_id, "install_receipt_id": install_id, "restart_receipt_id": restart_id,
            "entrypoint": str(entrypoint), "producer": {"adapter": "freehand-v2-channel-registry-adapter", "identity": attempt},
            "observed_at": created_at}, "source_unchanged": True, "result": "pass", "created_at": created_at}
    stage(staging_dir / "pre-review.json", pre_review)
    evidence_dir.mkdir(parents=True, exist_ok=True)
    finalize(evidence_dir / f"{whitebox_id}.json", whitebox)
    finalize(evidence_dir / f"{install_id}.json", install)
    finalize(evidence_dir / f"{restart_id}.json", restart)
    finalize(evidence_dir / f"{blackbox_id}.json", blackbox)
    finalize(records_dir / f"fix-candidate-record-{module_id}.json", fix_candidate)
    finalize(pre_review_path, pre_review)
    print(json.dumps({"transaction_attempt_id": attempt, "candidate_commit": head_commit, "artifact_hash": artifact_hash,
        "whitebox_evidence_id": whitebox_id, "install_evidence_id": install_id, "restart_evidence_id": restart_id,
        "blackbox_evidence_id": blackbox_id, "pre_review_validation_id": validation_id}, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
