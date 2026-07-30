use super::*;

#[derive(Clone, Copy)]
pub(super) enum MapDocumentKind {
    Function,
    Mainline,
    TestDesign,
}

impl MapDocumentKind {
    fn directory(self) -> &'static str {
        match self {
            Self::Function => "docs/function-maps",
            Self::Mainline => "docs/mainline-calls",
            Self::TestDesign => "docs/testing",
        }
    }

    fn document_extension(self) -> &'static str {
        match self {
            Self::Mainline => "json",
            Self::Function | Self::TestDesign => "md",
        }
    }
}

pub(super) fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{context} missing non-empty string `{field}`"))
}

pub(super) fn string_array(
    value: Option<&Value>,
    context: &str,
) -> Result<BTreeSet<String>, String> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{context} must be an array"))?;
    let mut output = BTreeSet::new();
    for value in values {
        let value = value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("{context} must contain only non-empty strings"))?;
        if !output.insert(value.to_owned()) {
            return Err(format!("{context} contains duplicate `{value}`"));
        }
    }
    Ok(output)
}

pub(super) fn inline_migration_node_ids(markdown: &str) -> BTreeSet<String> {
    markdown
        .split('`')
        .enumerate()
        .filter_map(|(index, value)| (index % 2 == 1).then_some(value))
        .filter(|value| {
            OPENMINIS_UI_MIGRATION_NODE_PREFIXES
                .iter()
                .any(|prefix| value.starts_with(prefix))
                && value.chars().all(|ch| {
                    ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.'
                })
        })
        .map(ToOwned::to_owned)
        .collect()
}

pub(super) type OpenMinisUiMermaidTopology = (BTreeSet<String>, BTreeSet<(String, String)>);
pub(super) type OpenMinisUiForwardEdge = (String, String, String, String);
pub(super) type OpenMinisUiReturnPath = (String, String, String);

pub(super) fn collect_regular_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| format!("inspect target path {}: {err}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "target path {} must not contain a symbolic link",
            path.display()
        ));
    }
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(format!(
            "target path {} is not a regular file or directory",
            path.display()
        ));
    }
    for entry in fs::read_dir(path).map_err(|err| format!("read {}: {err}", path.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        collect_regular_files(&entry.path(), files)?;
    }
    Ok(())
}

pub(super) fn repository_relative_path(raw: &str, context: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw);
    if raw.trim() != raw
        || raw.is_empty()
        || raw.contains('\\')
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "{context} must be a normalized repository-relative path: `{raw}`"
        ));
    }
    Ok(path.to_path_buf())
}

fn ensure_no_symlink_components(root: &Path, relative: &Path, context: &str) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|err| format!("{context} cannot inspect {}: {err}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{context} must not traverse a symbolic link: {}",
                current.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn existing_repository_path(
    root: &Path,
    raw: &str,
    context: &str,
) -> Result<PathBuf, String> {
    let relative = repository_relative_path(raw, context)?;
    ensure_no_symlink_components(root, &relative, context)?;
    let canonical_root = root
        .canonicalize()
        .map_err(|err| format!("canonicalize repository root {}: {err}", root.display()))?;
    let candidate = root
        .join(relative)
        .canonicalize()
        .map_err(|err| format!("{context} does not resolve inside repository: `{raw}`: {err}"))?;
    candidate
        .strip_prefix(&canonical_root)
        .map_err(|_| format!("{context} resolves outside repository: `{raw}`"))?;
    Ok(candidate)
}

pub(super) fn verify_openminis_ui_map_path(
    root: &Path,
    node_id: &str,
    _owner_feature_id: &str,
    kind: MapDocumentKind,
    raw: &str,
) -> Result<String, String> {
    let context = format!("OpenMinis UI migration node `{node_id}` map path");
    let relative = repository_relative_path(raw, &context)?;
    let parent = relative
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| format!("{context} has no canonical map directory: `{raw}`"))?;
    if parent != kind.directory()
        || relative.extension().and_then(|value| value.to_str()) != Some(kind.document_extension())
    {
        return Err(format!(
            "{context} must live directly under `{}` with a .{} extension: `{raw}`",
            kind.directory(),
            kind.document_extension()
        ));
    }
    let feature_id = relative
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{context} has no feature identity: `{raw}`"))?;
    let canonical = existing_repository_path(root, raw, &context)?;
    if !canonical.is_file() {
        return Err(format!("{context} is not a regular file: `{raw}`"));
    }
    let content =
        fs::read_to_string(&canonical).map_err(|err| format!("read {context} `{raw}`: {err}"))?;
    match kind {
        MapDocumentKind::Function => {
            for required in [
                format!("# Function Map: `{feature_id}`"),
                format!("- feature_id: `{feature_id}`"),
            ] {
                if !content.lines().any(|line| line.trim() == required) {
                    return Err(format!(
                        "{context} function-map self identity does not match `{feature_id}`: `{raw}`"
                    ));
                }
            }
        }
        MapDocumentKind::TestDesign => {
            for required in [
                format!("# Test Design: `{feature_id}`"),
                format!("- feature_id: `{feature_id}`"),
            ] {
                if !content.lines().any(|line| line.trim() == required) {
                    return Err(format!(
                        "{context} test-design self identity does not match `{feature_id}`: `{raw}`"
                    ));
                }
            }
        }
        MapDocumentKind::Mainline => {
            let value: Value = serde_json::from_str(&content)
                .map_err(|err| format!("parse {context} `{raw}`: {err}"))?;
            if value.get("feature_id").and_then(Value::as_str) != Some(feature_id)
                || value.get("mainline_call_doc").and_then(Value::as_str) != Some(raw)
                || value.get("function_map_doc").and_then(Value::as_str)
                    != Some(&format!("docs/function-maps/{feature_id}.md"))
                || value.get("test_design_doc").and_then(Value::as_str)
                    != Some(&format!("docs/testing/{feature_id}.md"))
            {
                return Err(format!(
                    "{context} mainline self identity does not match feature `{feature_id}` and path `{raw}`"
                ));
            }
        }
    }
    Ok(feature_id.to_owned())
}

pub(super) fn verify_openminis_ui_map_documents(
    root: &Path,
    node_id: &str,
    owner_feature_id: &str,
    touched_features: &BTreeSet<String>,
    node: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let mut feature_sets = Vec::new();
    for (field, kind) in [
        ("function_map_docs", MapDocumentKind::Function),
        ("mainline_call_docs", MapDocumentKind::Mainline),
        ("test_design_docs", MapDocumentKind::TestDesign),
    ] {
        let paths = string_array(
            node.get(field),
            &format!("migration node {node_id} {field}"),
        )?;
        if paths.is_empty() {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` has no `{field}` candidates"
            ));
        }
        let mut features = BTreeSet::new();
        for path in paths {
            features.insert(verify_openminis_ui_map_path(
                root,
                node_id,
                owner_feature_id,
                kind,
                &path,
            )?);
        }
        feature_sets.push((field, features));
    }
    let expected = &feature_sets[0].1;
    if feature_sets
        .iter()
        .any(|(_, features)| features != expected)
    {
        return Err(format!(
            "OpenMinis UI migration node `{node_id}` map feature-id drift: {:?}",
            feature_sets
        ));
    }
    if !expected.contains(owner_feature_id) {
        return Err(format!(
            "OpenMinis UI migration node `{node_id}` map documents do not include owner feature `{owner_feature_id}`"
        ));
    }
    if expected != touched_features {
        return Err(format!(
            "OpenMinis UI migration node `{node_id}` map feature ids must equal touched_feature_ids: maps={expected:?}, touched={touched_features:?}"
        ));
    }
    Ok(())
}
