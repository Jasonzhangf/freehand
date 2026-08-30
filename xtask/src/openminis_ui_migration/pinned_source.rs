use super::*;

pub(super) fn verify_openminis_ui_pinned_source(
    root: &Path,
    source_repository: &serde_json::Map<String, Value>,
    nodes: &[Value],
) -> Result<(), String> {
    let path_hint = required_string(
        source_repository,
        "path_hint",
        "OpenMinis UI migration source_repository",
    )?;
    if path_hint != "external/OpenMinis" {
        return Err(format!(
            "OpenMinis UI migration source path must be `external/OpenMinis`, got `{path_hint}`"
        ));
    }
    let commit = required_string(
        source_repository,
        "commit",
        "OpenMinis UI migration source_repository",
    )?;
    verify_openminis_ui_ci_checkout(root, commit)?;
    let repository = root.join(path_hint);
    if !repository.is_dir() {
        return Err(format!(
            "OpenMinis UI migration pinned source checkout is missing at `{path_hint}`"
        ));
    }
    let head = git_stdout(&repository, &["rev-parse", "HEAD"])?;
    if head != commit {
        return Err(format!(
            "OpenMinis UI migration pinned source HEAD drift: expected `{commit}`, got `{head}`"
        ));
    }
    let commit_type = git_stdout(&repository, &["cat-file", "-t", commit])?;
    if commit_type != "commit" {
        return Err(format!(
            "OpenMinis UI migration pinned object `{commit}` is not a commit"
        ));
    }

    for node in nodes {
        let node = node
            .as_object()
            .ok_or_else(|| "OpenMinis UI migration nodes must contain objects".to_owned())?;
        let node_id = required_string(node, "node_id", "OpenMinis UI migration node")?;
        let source_paths = string_array(
            node.get("source_paths"),
            &format!("migration node {node_id} source_paths"),
        )?;
        let source_symbols = string_array(
            node.get("source_symbols"),
            &format!("migration node {node_id} source_symbols"),
        )?;
        let mut blob_paths = BTreeSet::new();
        for source_path in &source_paths {
            let is_excluded_path = |path: &str| {
                let normalized = format!("/{}", path.to_ascii_lowercase().replace('\\', "/"));
                OPENMINIS_UI_EXCLUDED_SOURCE_PATH_TOKENS
                    .iter()
                    .any(|token| normalized.contains(token))
            };
            if is_excluded_path(source_path) {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` includes excluded source path `{source_path}`"
                ));
            }
            let output = git_stdout(
                &repository,
                &["ls-tree", "-r", "--name-only", commit, "--", source_path],
            )?;
            let matches = output
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if matches.is_empty() {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` source path `{source_path}` does not exist at pinned commit `{commit}`"
                ));
            }
            if let Some(excluded_path) = matches.iter().find(|path| is_excluded_path(path)) {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` recursively resolves excluded source path `{excluded_path}` from `{source_path}`"
                ));
            }
            blob_paths.extend(matches);
        }
        let mut declarations = BTreeMap::<String, Vec<DeclarationOccurrence>>::new();
        for blob_path in blob_paths {
            let object = format!("{commit}:{blob_path}");
            let source = git_stdout(&repository, &["show", &object])?;
            for declaration in declared_symbols(Path::new(&blob_path), &source)? {
                declarations
                    .entry(declaration.name.clone())
                    .or_default()
                    .push(declaration);
            }
        }
        for symbol in source_symbols {
            let lower = symbol.to_ascii_lowercase();
            if OPENMINIS_UI_EXCLUDED_SOURCE_TOKENS
                .iter()
                .any(|token| lower.contains(token))
            {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` includes excluded source symbol `{symbol}`"
                ));
            }
            let resolved = declarations.get(&symbol).map(Vec::as_slice).unwrap_or(&[]);
            if resolved.len() != 1 {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` source symbol `{symbol}` must resolve as exactly one declaration at pinned commit `{commit}`, found {}",
                    resolved.len()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn verify_openminis_ui_ci_checkout(root: &Path, commit: &str) -> Result<(), String> {
    let workflow_dir = root.join(".github/workflows");
    let mut workflow_paths = fs::read_dir(&workflow_dir)
        .map_err(|err| format!("read {}: {err}", workflow_dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("read {} entry: {err}", workflow_dir.display()))?;
    workflow_paths.sort();
    let mut verified_gate_workflows = Vec::new();
    for workflow_path in workflow_paths {
        if !matches!(
            workflow_path.extension().and_then(|value| value.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let workflow_source = fs::read_to_string(&workflow_path)
            .map_err(|err| format!("read {}: {err}", workflow_path.display()))?;
        let workflow: serde_yaml::Value = serde_yaml::from_str(&workflow_source)
            .map_err(|err| format!("parse {} as YAML: {err}", workflow_path.display()))?;
        if verify_gate_workflow(&workflow_path, &workflow, commit)? {
            verified_gate_workflows.push(workflow_path);
        }
    }
    let ci_path = root.join(".github/workflows/ci.yml");
    if !verified_gate_workflows.contains(&ci_path) {
        return Err("CI workflow must contain exactly one `make ci` gate job".to_owned());
    }
    Ok(())
}

fn verify_gate_workflow(
    workflow_path: &Path,
    workflow: &serde_yaml::Value,
    commit: &str,
) -> Result<bool, String> {
    let workflow_name = workflow_path.to_string_lossy();
    let jobs = workflow
        .as_mapping()
        .and_then(|workflow| yaml_mapping_value(workflow, "jobs"))
        .and_then(serde_yaml::Value::as_mapping)
        .ok_or_else(|| format!("{workflow_name} must contain a `jobs` mapping"))?;
    let mut gate_jobs = Vec::new();
    for (job_id, job) in jobs {
        let job_id = job_id
            .as_str()
            .ok_or_else(|| format!("{workflow_name} job ids must be strings"))?;
        let Some(steps) = job
            .as_mapping()
            .and_then(|job| yaml_mapping_value(job, "steps"))
            .and_then(serde_yaml::Value::as_sequence)
        else {
            continue;
        };
        let full_gate_count = steps
            .iter()
            .filter(|step| {
                step.as_mapping()
                    .and_then(|step| yaml_mapping_value(step, "run"))
                    .and_then(serde_yaml::Value::as_str)
                    .is_some_and(|run| run.trim() == "make ci")
            })
            .count();
        if full_gate_count > 0 {
            gate_jobs.push((job_id, steps, full_gate_count));
        }
    }
    if gate_jobs.is_empty() {
        return Ok(false);
    }
    if gate_jobs.len() != 1 || gate_jobs[0].2 != 1 {
        return Err(format!(
            "{workflow_name} must contain exactly one job with exactly one `make ci` step, found {gate_jobs:?}"
        ));
    }
    let (gate_job_id, gate_steps, _) = gate_jobs[0];
    let gate_step_index = gate_steps
        .iter()
        .position(|step| {
            step.as_mapping()
                .and_then(|step| yaml_mapping_value(step, "run"))
                .and_then(serde_yaml::Value::as_str)
                .is_some_and(|run| run.trim() == "make ci")
        })
        .expect("gate job contains make ci");
    let checkout_steps = gate_steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| step.as_mapping().map(|step| (index, step)))
        .filter(|(_, step)| {
            yaml_mapping_value(step, "uses").and_then(serde_yaml::Value::as_str)
                == Some("actions/checkout@v4")
                && yaml_mapping_value(step, "with")
                    .and_then(serde_yaml::Value::as_mapping)
                    .and_then(|with| yaml_mapping_value(with, "repository"))
                    .and_then(serde_yaml::Value::as_str)
                    == Some("OpenMinis/OpenMinis")
        })
        .collect::<Vec<_>>();
    if checkout_steps.len() != 1 {
        return Err(format!(
            "{workflow_name} gate job `{gate_job_id}` must contain exactly one actions/checkout@v4 step for OpenMinis/OpenMinis, found {}",
            checkout_steps.len()
        ));
    }
    let (checkout_index, step) = checkout_steps[0];
    if checkout_index >= gate_step_index {
        return Err(format!(
            "{workflow_name} OpenMinis checkout must run before `make ci`"
        ));
    }
    let with = yaml_mapping_value(step, "with")
        .and_then(serde_yaml::Value::as_mapping)
        .ok_or_else(|| format!("{workflow_name} OpenMinis checkout must contain `with`"))?;
    if yaml_mapping_value(with, "ref").and_then(serde_yaml::Value::as_str) != Some(commit) {
        return Err(format!(
            "{workflow_name} OpenMinis checkout ref must match manifest commit `{commit}`"
        ));
    }
    if yaml_mapping_value(with, "path").and_then(serde_yaml::Value::as_str)
        != Some("external/OpenMinis")
    {
        return Err(format!(
            "{workflow_name} OpenMinis checkout path must be `external/OpenMinis`"
        ));
    }
    let swift_steps = gate_steps
        .iter()
        .enumerate()
        .filter(|(_, step)| {
            step.as_mapping()
                .and_then(|step| yaml_mapping_value(step, "uses"))
                .and_then(serde_yaml::Value::as_str)
                == Some("swift-actions/setup-swift@v2")
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if swift_steps.len() != 1 || swift_steps[0] >= gate_step_index {
        return Err(format!(
            "{workflow_name} gate job `{gate_job_id}` must install exactly one swift-actions/setup-swift@v2 before `make ci`"
        ));
    }
    Ok(true)
}

fn yaml_mapping_value<'a>(
    mapping: &'a serde_yaml::Mapping,
    key: &str,
) -> Option<&'a serde_yaml::Value> {
    mapping.get(serde_yaml::Value::String(key.to_owned()))
}

pub(super) fn git_stdout(repository: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .map_err(|err| format!("run git in {}: {err}", repository.display()))?;
    if !output.status.success() {
        return Err(format!(
            "git -C {} {} failed: {}",
            repository.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|err| format!("git output was not UTF-8: {err}"))
}
