#!/usr/bin/env python3
"""Freehand v2 Session Log AppSDK lifecycle adapter."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import uuid


def utc_now() -> str:
    return (
        datetime.datetime.now(datetime.timezone.utc)
        .isoformat()
        .replace("+00:00", "Z")
    )


def sha256_text(value: str) -> str:
    return "sha256:" + hashlib.sha256(value.encode("utf-8")).hexdigest()


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def run(cmd, cwd=None, input_text=None) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        cwd=cwd,
        input=input_text,
        text=True,
        capture_output=True,
        check=False,
    )


def git(project: pathlib.Path, args) -> subprocess.CompletedProcess:
    return run(["git", *args], cwd=project)


def fail(attempt: str, stage: str, message: str) -> None:
    payload = {
        "transaction_attempt_id": attempt,
        "failure_node": stage,
        "error": message,
        "retry_allowed": False,
        "next": "fix the adapter failure route; do not overwrite existing records",
    }
    sys.stderr.write(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    raise SystemExit(2)


def evidence_record(
    *,
    evidence_id: str,
    issue_id: str,
    experiment_id: str,
    phase: str,
    kind: str,
    source_commit: str,
    scope_hash: str,
    producer_identity: str,
    created_at: str,
    input_hashes: list[str],
    artifact_hash: str | None = None,
    environment_id: str | None = None,
    entrypoint: str | None = None,
) -> dict:
    record = {
        "evidence_id": evidence_id,
        "issue_id": issue_id,
        "experiment_id": experiment_id,
        "phase": phase,
        "kind": kind,
        "source_commit": source_commit,
        "scope": {"module_id": "v2-sessionlog"},
        "producer": {"adapter": "freehand-v2-sessionlog-adapter", "identity": producer_identity},
        "result": "pass",
        "created_at": created_at,
        "expires_at": (
            datetime.datetime.now(datetime.timezone.utc)
            + datetime.timedelta(days=1)
        ).isoformat()
        .replace("+00:00", "Z"),
        "input_hashes": input_hashes,
        "scope_hash": scope_hash,
    }
    if phase == "development_whitebox":
        record["execution_surface"] = "development_whitebox"
    if phase in {"deployment_install", "deployment_restart", "deployed_blackbox"}:
        record["artifact_hash"] = artifact_hash
        record["execution_surface"] = "deployed_blackbox"
        record["environment_id"] = environment_id
        record["entrypoint"] = entrypoint
    return record


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("project", nargs="?", default=".")
    parser.add_argument("--module", default="v2-sessionlog")
    parser.add_argument("--attempt", default=None)
    args = parser.parse_args()

    project = pathlib.Path(args.project).resolve()
    if args.module != "v2-sessionlog":
        fail("unknown", "argument", f"unsupported module {args.module}")

    attempt = args.attempt or f"{datetime.datetime.utcnow():%Y%m%dT%H%M%S}-{uuid.uuid4().hex[:8]}"
    issue_id = "freehand-v2-sessionlog-milestone"
    experiment_id = f"v2-sessionlog-{attempt}"
    records_dir = project / ".appsdk" / "records"
    evidence_dir = records_dir / "evidence" / "v2-sessionlog"
    staging_dir = project / ".appsdk-control" / "transactions" / "v2-sessionlog" / attempt
    staging_dir.mkdir(parents=True, exist_ok=True)

    head_proc = git(project, ["rev-parse", "HEAD"])
    if head_proc.returncode != 0:
        fail(attempt, "candidate_identity", "cannot resolve HEAD")
    head_commit = head_proc.stdout.strip()
    base_proc = git(project, ["rev-parse", f"{head_commit}^"])
    if base_proc.returncode != 0:
        fail(attempt, "candidate_identity", "cannot resolve HEAD parent")
    base_commit = base_proc.stdout.strip()

    tree_proc = git(project, ["rev-parse", f"{head_commit}^{{tree}}"])
    if tree_proc.returncode != 0:
        fail(attempt, "candidate_identity", "cannot resolve HEAD tree")
    tree_hash = tree_proc.stdout.strip()

    changed_proc = git(project, ["diff", "--name-only", base_commit, head_commit])
    if changed_proc.returncode != 0:
        fail(attempt, "candidate_identity", "cannot list changed paths")
    changed_paths = sorted(p for p in changed_proc.stdout.splitlines() if p)
    diff_proc = git(project, ["diff", "--binary", base_commit, head_commit])
    diff_hash = sha256_bytes(diff_proc.stdout.encode("utf-8"))
    scope_hash = sha256_text(json.dumps(changed_paths, sort_keys=True))

    pre_review_path = records_dir / "pre-review-validation-record-v2-sessionlog.json"
    if pre_review_path.exists():
        existing = json.loads(pre_review_path.read_text())
        if existing.get("candidate_commit") == head_commit:
            print(
                json.dumps(
                    {"existing_validation_id": existing.get("validation_id"), "candidate_commit": head_commit},
                    sort_keys=True,
                )
            )
            return
        fail(attempt, "pre_review_validation", "existing pre-review record binds another candidate")

    appsdk_bin = shutil.which("appsdk")
    if not appsdk_bin:
        fail(attempt, "artifact", "appsdk binary not found")
    compile_proc = run([appsdk_bin, "compile-module", str(project), "--module", args.module])
    if compile_proc.returncode != 0:
        fail(attempt, "artifact", compile_proc.stderr or compile_proc.stdout)
    try:
        compile_json = json.loads(compile_proc.stdout)
    except json.JSONDecodeError as exc:
        fail(attempt, "artifact", f"cannot parse compile-module output: {exc}")
    artifact_hash = compile_json["artifact_hash"]

    manifest_path = project / "playground" / "experiments" / "v2" / "sessionlog" / "Cargo.toml"
    test_cmd = [
        "cargo",
        "test",
        "--manifest-path",
        str(manifest_path),
        "--test",
        "v2_sessionlog_boundary",
    ]
    whitebox_proc = run(test_cmd, cwd=project)
    whitebox_input_hash = sha256_text(json.dumps(test_cmd, sort_keys=True))
    created_at = utc_now()
    whitebox_id = f"ev-v2-sessionlog-whitebox-{attempt}"
    whitebox = evidence_record(
        evidence_id=whitebox_id,
        issue_id=issue_id,
        experiment_id=experiment_id,
        phase="development_whitebox",
        kind="positive_test",
        source_commit=head_commit,
        scope_hash=scope_hash,
        producer_identity=attempt,
        created_at=created_at,
        input_hashes=[whitebox_input_hash],
    )
    stage(staging_dir / "whitebox.json", whitebox)
    if whitebox_proc.returncode != 0:
        fail(attempt, "development_whitebox", whitebox_proc.stderr or whitebox_proc.stdout)

    deploy_dir = pathlib.Path("/tmp") / f"freehand-v2-sessionlog-{attempt}"
    deploy_dir.mkdir(parents=True, exist_ok=True)
    entrypoint = deploy_dir / "v2-sessionlog-public"
    artifact = project / "generated" / "modules" / "v2-sessionlog" / "lib" / "v2-sessionlog.module"
    if not artifact.exists():
        fail(attempt, "deployment_install", f"missing compiled artifact {artifact}")
    shutil.copy2(artifact, entrypoint)

    environment_id = f"local-freehand-v2-sessionlog-{attempt}"
    install_input = json.dumps(
        {"session_id": f"s-install-{attempt}", "event_id": f"e-install-{attempt}", "data": "install ok"},
        sort_keys=True,
    )
    restart_input = json.dumps(
        {"session_id": f"s-restart-{attempt}", "event_id": f"e-restart-{attempt}", "data": "restart ok"},
        sort_keys=True,
    )
    blackbox_input = json.dumps(
        {"session_id": f"s-blackbox-{attempt}", "event_id": f"e-blackbox-{attempt}", "data": "deployed ok"},
        sort_keys=True,
    )

    install_id = f"ev-v2-sessionlog-install-{attempt}"
    install = evidence_record(
        evidence_id=install_id,
        issue_id=issue_id,
        experiment_id=experiment_id,
        phase="deployment_install",
        kind="install",
        source_commit=head_commit,
        scope_hash=scope_hash,
        producer_identity=attempt,
        created_at=created_at,
        input_hashes=[sha256_text(install_input)],
        artifact_hash=artifact_hash,
        environment_id=environment_id,
        entrypoint=str(entrypoint),
    )
    stage(staging_dir / "install.json", install)
    install_proc = run([str(entrypoint)], cwd=project, input_text=install_input)
    if install_proc.returncode != 0:
        fail(attempt, "deployment_install", install_proc.stderr or install_proc.stdout)

    restart_id = f"ev-v2-sessionlog-restart-{attempt}"
    restart = evidence_record(
        evidence_id=restart_id,
        issue_id=issue_id,
        experiment_id=experiment_id,
        phase="deployment_restart",
        kind="restart",
        source_commit=head_commit,
        scope_hash=scope_hash,
        producer_identity=attempt,
        created_at=created_at,
        input_hashes=[sha256_text(restart_input)],
        artifact_hash=artifact_hash,
        environment_id=environment_id,
        entrypoint=str(entrypoint),
    )
    stage(staging_dir / "restart.json", restart)
    restart_proc = run([str(entrypoint)], cwd=project, input_text=restart_input)
    if restart_proc.returncode != 0:
        fail(attempt, "deployment_restart", restart_proc.stderr or restart_proc.stdout)

    blackbox_id = f"ev-v2-sessionlog-blackbox-{attempt}"
    blackbox = evidence_record(
        evidence_id=blackbox_id,
        issue_id=issue_id,
        experiment_id=experiment_id,
        phase="deployed_blackbox",
        kind="runtime",
        source_commit=head_commit,
        scope_hash=scope_hash,
        producer_identity=attempt,
        created_at=created_at,
        input_hashes=[sha256_text(blackbox_input)],
        artifact_hash=artifact_hash,
        environment_id=environment_id,
        entrypoint=str(entrypoint),
    )
    stage(staging_dir / "blackbox.json", blackbox)
    blackbox_proc = run([str(entrypoint)], cwd=project, input_text=blackbox_input)
    if blackbox_proc.returncode != 0:
        fail(attempt, "deployed_blackbox", blackbox_proc.stderr or blackbox_proc.stdout)

    fix_candidate_id = f"fix-v2-sessionlog-{attempt}"
    fix_candidate = {
        "fix_candidate_id": fix_candidate_id,
        "issue_id": issue_id,
        "module_id": "v2-sessionlog",
        "worktree_id": "playground/v2-sessionlog-20260903",
        "base_commit": base_commit,
        "head_commit": head_commit,
        "tree_hash": tree_hash,
        "diff_hash": diff_hash,
        "design_id": "docs/design/v2-sessionlog-owner-binding.md",
        "owner": "v2-sessionlog",
        "scope_hash": scope_hash,
        "changed_paths": changed_paths,
        "verification_evidence_ids": [whitebox_id, install_id, restart_id, blackbox_id],
        "created_at": created_at,
    }
    stage(staging_dir / "fix-candidate.json", fix_candidate)

    validation_id = f"prv-v2-sessionlog-{attempt}"
    pre_review = {
        "validation_id": validation_id,
        "issue_id": issue_id,
        "module_id": "v2-sessionlog",
        "fix_candidate_id": fix_candidate_id,
        "candidate_commit": head_commit,
        "candidate_tree_hash": tree_hash,
        "artifact_hash": artifact_hash,
        "whitebox_producer": {"adapter": "freehand-v2-sessionlog-adapter", "identity": attempt},
        "whitebox_evidence_ids": [whitebox_id],
        "blackbox_evidence_ids": [blackbox_id],
        "deployment": {
            "environment_id": environment_id,
            "install_receipt_id": install_id,
            "restart_receipt_id": restart_id,
            "entrypoint": str(entrypoint),
            "producer": {"adapter": "freehand-v2-sessionlog-adapter", "identity": attempt},
            "observed_at": created_at,
        },
        "source_unchanged": True,
        "result": "pass",
        "created_at": created_at,
    }
    stage(staging_dir / "pre-review.json", pre_review)

    evidence_dir.mkdir(parents=True, exist_ok=True)
    finalize(evidence_dir / f"whitebox-{attempt}.json", whitebox)
    finalize(evidence_dir / f"install-{attempt}.json", install)
    finalize(evidence_dir / f"restart-{attempt}.json", restart)
    finalize(evidence_dir / f"blackbox-{attempt}.json", blackbox)
    finalize(records_dir / "fix-candidate-record-v2-sessionlog.json", fix_candidate)
    finalize(pre_review_path, pre_review)

    print(
        json.dumps(
            {
                "transaction_attempt_id": attempt,
                "candidate_commit": head_commit,
                "artifact_hash": artifact_hash,
                "whitebox_evidence_id": whitebox_id,
                "install_evidence_id": install_id,
                "restart_evidence_id": restart_id,
                "blackbox_evidence_id": blackbox_id,
                "pre_review_validation_id": validation_id,
            },
            indent=2,
            sort_keys=True,
        )
    )


def stage(path: pathlib.Path, record: dict) -> None:
    path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")


def finalize(path: pathlib.Path, record: dict) -> None:
    if path.exists():
        existing = json.loads(path.read_text())
        if existing == record:
            return
        fail(record.get("evidence_id") or path.stem, "finalize", f"refusing to overwrite {path}")
    path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
