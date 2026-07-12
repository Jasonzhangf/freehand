//! Deterministic instruction capability manifest compiler for Freehand.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

const SCHEMA_VERSION: u32 = 1;
const GLOBAL_AGENTS_FILENAME: &str = "AGENTS.md";
const LOCAL_AGENTS_FILENAME: &str = "AGENTS.md";
const SKILLS_DIR: &str = "skills";
const LOCAL_AGENTS_DIR: &str = ".agents";
const SKILL_FILENAME: &str = "SKILL.md";
const MAX_SCAN_DEPTH: usize = 6;
const MAX_SKILL_NAME_LEN: usize = 64;
const MAX_SKILL_DESCRIPTION_LEN: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionCapabilityCompileInput {
    pub freehand_home: PathBuf,
    pub cwd: PathBuf,
    pub project_root_markers: Vec<String>,
}

impl InstructionCapabilityCompileInput {
    pub fn new(freehand_home: impl Into<PathBuf>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            freehand_home: freehand_home.into(),
            cwd: cwd.into(),
            project_root_markers: vec![".git".to_owned(), "Cargo.toml".to_owned()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionCapabilityManifest {
    pub schema_version: u32,
    pub generated_by: String,
    pub freehand_home: String,
    pub cwd: String,
    pub project_root: String,
    pub agents: Vec<AgentsMdCapability>,
    pub skills: Vec<SkillCapability>,
    pub errors: Vec<InstructionCapabilityErrorRecord>,
    pub manifest_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionCapabilityScope {
    Global,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentsMdCapability {
    pub scope: InstructionCapabilityScope,
    pub path: String,
    pub directory: String,
    pub precedence: u32,
    pub content_bytes: u64,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillCapability {
    pub scope: InstructionCapabilityScope,
    pub name: String,
    pub description: String,
    pub path: String,
    pub root: String,
    pub precedence: u32,
    pub content_bytes: u64,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionCapabilityErrorRecord {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstructionCapabilityError {
    #[error("cwd `{0}` is not a directory")]
    CwdNotDirectory(String),
    #[error("failed to read `{path}`: {message}")]
    ReadFile { path: String, message: String },
    #[error("failed to write `{path}`: {message}")]
    WriteFile { path: String, message: String },
    #[error("failed to render manifest json: {0}")]
    RenderManifest(String),
}

pub fn compile_instruction_capability_manifest(
    input: InstructionCapabilityCompileInput,
) -> Result<InstructionCapabilityManifest, InstructionCapabilityError> {
    if !input.cwd.is_dir() {
        return Err(InstructionCapabilityError::CwdNotDirectory(
            input.cwd.display().to_string(),
        ));
    }

    let freehand_home = normalize_path(&input.freehand_home);
    let cwd = normalize_path(&input.cwd);
    let project_root = find_project_root(&cwd, &input.project_root_markers);
    let local_dirs = dirs_between(&project_root, &cwd);
    let mut agents = Vec::new();
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    let mut seen_agents = BTreeSet::new();
    let mut seen_skills = BTreeSet::new();

    let global_agents_path = freehand_home.join(GLOBAL_AGENTS_FILENAME);
    if global_agents_path.is_file() {
        match agents_md_capability(InstructionCapabilityScope::Global, &global_agents_path, 0) {
            Ok(entry) => {
                if seen_agents.insert(entry.path.clone()) {
                    agents.push(entry);
                }
            }
            Err(err) => errors.push(err),
        }
    }

    let mut precedence = 10_u32;
    for dir in &local_dirs {
        let path = dir.join(LOCAL_AGENTS_FILENAME);
        if path.is_file() {
            match agents_md_capability(InstructionCapabilityScope::Local, &path, precedence) {
                Ok(entry) => {
                    if seen_agents.insert(entry.path.clone()) {
                        agents.push(entry);
                    }
                }
                Err(err) => errors.push(err),
            }
        }
        precedence = precedence.saturating_add(1);
    }

    let global_skill_root = freehand_home.join(SKILLS_DIR);
    collect_skills(
        InstructionCapabilityScope::Global,
        &global_skill_root,
        0,
        &mut skills,
        &mut errors,
        &mut seen_skills,
    );

    let mut local_skill_precedence = 10_u32;
    for dir in &local_dirs {
        collect_skills(
            InstructionCapabilityScope::Local,
            &dir.join(LOCAL_AGENTS_DIR).join(SKILLS_DIR),
            local_skill_precedence,
            &mut skills,
            &mut errors,
            &mut seen_skills,
        );
        local_skill_precedence = local_skill_precedence.saturating_add(1);
    }

    agents.sort_by(|left, right| {
        left.precedence
            .cmp(&right.precedence)
            .then_with(|| left.path.cmp(&right.path))
    });
    skills.sort_by(|left, right| {
        left.precedence
            .cmp(&right.precedence)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path.cmp(&right.path))
    });
    errors.sort_by(|left, right| left.path.cmp(&right.path));

    let fingerprint = manifest_fingerprint(&agents, &skills, &errors);
    Ok(InstructionCapabilityManifest {
        schema_version: SCHEMA_VERSION,
        generated_by: "freehand-instructions".to_owned(),
        freehand_home: path_string(&freehand_home),
        cwd: path_string(&cwd),
        project_root: path_string(&project_root),
        agents,
        skills,
        errors,
        manifest_fingerprint: fingerprint,
    })
}

pub fn write_instruction_capability_manifest(
    manifest: &InstructionCapabilityManifest,
    path: impl AsRef<Path>,
) -> Result<(), InstructionCapabilityError> {
    let path = path.as_ref();
    let payload = serde_json::to_string_pretty(manifest)
        .map_err(|err| InstructionCapabilityError::RenderManifest(err.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| InstructionCapabilityError::WriteFile {
            path: parent.display().to_string(),
            message: err.to_string(),
        })?;
    }
    fs::write(path, payload).map_err(|err| InstructionCapabilityError::WriteFile {
        path: path.display().to_string(),
        message: err.to_string(),
    })
}

pub fn render_instruction_capability_context(
    manifest: &InstructionCapabilityManifest,
) -> Result<String, InstructionCapabilityError> {
    let agents = manifest
        .agents
        .iter()
        .map(|entry| {
            let content = fs::read_to_string(&entry.path).map_err(|err| {
                InstructionCapabilityError::ReadFile {
                    path: entry.path.clone(),
                    message: err.to_string(),
                }
            })?;
            Ok(json!({
                "scope": entry.scope,
                "path": entry.path,
                "directory": entry.directory,
                "precedence": entry.precedence,
                "content_bytes": entry.content_bytes,
                "content_hash": entry.content_hash,
                "content": content
            }))
        })
        .collect::<Result<Vec<_>, InstructionCapabilityError>>()?;
    let skills = manifest
        .skills
        .iter()
        .map(|entry| {
            let content = fs::read_to_string(&entry.path).map_err(|err| {
                InstructionCapabilityError::ReadFile {
                    path: entry.path.clone(),
                    message: err.to_string(),
                }
            })?;
            Ok(json!({
                "scope": entry.scope,
                "name": entry.name,
                "description": entry.description,
                "path": entry.path,
                "root": entry.root,
                "precedence": entry.precedence,
                "content_bytes": entry.content_bytes,
                "content_hash": entry.content_hash,
                "content": content
            }))
        })
        .collect::<Result<Vec<_>, InstructionCapabilityError>>()?;
    let errors = manifest
        .errors
        .iter()
        .map(|entry| {
            json!({
                "path": entry.path,
                "message": entry.message
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": 1,
        "purpose": "Compiled Freehand instruction capability content admitted through typed request_context. Runtime/provider code must not scan AGENTS.md or skills directly.",
        "manifest_fingerprint": manifest.manifest_fingerprint,
        "freehand_home": manifest.freehand_home,
        "cwd": manifest.cwd,
        "project_root": manifest.project_root,
        "agents": agents,
        "skills": skills,
        "errors": errors
    });
    let payload_text = serde_json::to_string_pretty(&payload)
        .map_err(|err| InstructionCapabilityError::RenderManifest(err.to_string()))?;
    Ok(format!(
        "Compiled Freehand instruction capability content. Treat this as typed framework instruction capability context, not as task/user payload.\n<freehand_instruction_capability>\n{payload_text}\n</freehand_instruction_capability>"
    ))
}

fn agents_md_capability(
    scope: InstructionCapabilityScope,
    path: &Path,
    precedence: u32,
) -> Result<AgentsMdCapability, InstructionCapabilityErrorRecord> {
    let content = read_text_for_record(path)?;
    Ok(AgentsMdCapability {
        scope,
        path: path_string(&normalize_path(path)),
        directory: path
            .parent()
            .map(normalize_path)
            .map(|path| path_string(&path))
            .unwrap_or_default(),
        precedence,
        content_bytes: content.len() as u64,
        content_hash: fnv1a_hex(content.as_bytes()),
    })
}

fn collect_skills(
    scope: InstructionCapabilityScope,
    root: &Path,
    precedence: u32,
    skills: &mut Vec<SkillCapability>,
    errors: &mut Vec<InstructionCapabilityErrorRecord>,
    seen_skills: &mut BTreeSet<String>,
) {
    if !root.is_dir() {
        return;
    }

    let mut skill_paths = WalkDir::new(root)
        .follow_links(true)
        .max_depth(MAX_SCAN_DEPTH)
        .into_iter()
        .filter_entry(visible_entry)
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && entry.file_name() == SKILL_FILENAME)
        .map(|entry| normalize_path(entry.path()))
        .collect::<Vec<_>>();
    skill_paths.sort();

    let normalized_root = normalize_path(root);
    for path in skill_paths {
        match skill_capability(scope, &normalized_root, &path, precedence) {
            Ok(skill) => {
                if seen_skills.insert(skill.path.clone()) {
                    skills.push(skill);
                }
            }
            Err(err) => errors.push(err),
        }
    }
}

fn visible_entry(entry: &DirEntry) -> bool {
    entry.depth() == 0
        || entry
            .file_name()
            .to_str()
            .is_some_and(|name| !name.starts_with('.'))
}

fn skill_capability(
    scope: InstructionCapabilityScope,
    root: &Path,
    path: &Path,
    precedence: u32,
) -> Result<SkillCapability, InstructionCapabilityErrorRecord> {
    let content = read_text_for_record(path)?;
    let metadata = parse_skill_frontmatter(path, &content)?;
    Ok(SkillCapability {
        scope,
        name: metadata.name,
        description: metadata.description,
        path: path_string(path),
        root: path_string(root),
        precedence,
        content_bytes: content.len() as u64,
        content_hash: fnv1a_hex(content.as_bytes()),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

fn parse_skill_frontmatter(
    path: &Path,
    content: &str,
) -> Result<SkillFrontmatter, InstructionCapabilityErrorRecord> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Err(record_error(
            path,
            "missing YAML frontmatter delimited by ---",
        ));
    }
    let mut fields = BTreeMap::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            fields.insert(key.trim().to_owned(), unquote(value.trim()).to_owned());
        }
    }
    let name = fields
        .remove("name")
        .ok_or_else(|| record_error(path, "missing field `name`"))?;
    let description = fields
        .remove("description")
        .ok_or_else(|| record_error(path, "missing field `description`"))?;
    validate_skill_field(path, "name", &name, MAX_SKILL_NAME_LEN)?;
    validate_skill_field(path, "description", &description, MAX_SKILL_DESCRIPTION_LEN)?;
    Ok(SkillFrontmatter { name, description })
}

fn validate_skill_field(
    path: &Path,
    field: &'static str,
    value: &str,
    max_len: usize,
) -> Result<(), InstructionCapabilityErrorRecord> {
    if value.trim().is_empty() {
        return Err(record_error(
            path,
            format!("field `{field}` must not be empty"),
        ));
    }
    if value.chars().count() > max_len {
        return Err(record_error(
            path,
            format!("field `{field}` exceeds max length {max_len}"),
        ));
    }
    Ok(())
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn read_text_for_record(path: &Path) -> Result<String, InstructionCapabilityErrorRecord> {
    fs::read_to_string(path).map_err(|err| InstructionCapabilityErrorRecord {
        path: path_string(&normalize_path(path)),
        message: format!("failed to read file: {err}"),
    })
}

fn record_error(path: &Path, message: impl Into<String>) -> InstructionCapabilityErrorRecord {
    InstructionCapabilityErrorRecord {
        path: path_string(&normalize_path(path)),
        message: message.into(),
    }
}

fn find_project_root(cwd: &Path, markers: &[String]) -> PathBuf {
    for ancestor in cwd.ancestors() {
        if markers.iter().any(|marker| ancestor.join(marker).exists()) {
            return normalize_path(ancestor);
        }
    }
    normalize_path(cwd)
}

fn dirs_between(project_root: &Path, cwd: &Path) -> Vec<PathBuf> {
    let mut dirs = cwd
        .ancestors()
        .scan(false, |done, dir| {
            if *done {
                None
            } else {
                if dir == project_root {
                    *done = true;
                }
                Some(normalize_path(dir))
            }
        })
        .collect::<Vec<_>>();
    dirs.reverse();
    dirs
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref()
        .canonicalize()
        .unwrap_or_else(|_| path.as_ref().to_path_buf())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn manifest_fingerprint(
    agents: &[AgentsMdCapability],
    skills: &[SkillCapability],
    errors: &[InstructionCapabilityErrorRecord],
) -> String {
    let materialized = serde_json::to_string(&(agents, skills, errors)).unwrap_or_default();
    fnv1a_hex(materialized.as_bytes())
}

fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("freehand-instructions-{nanos}-{counter}"));
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir parent");
        fs::write(path, content).expect("write");
    }

    #[test]
    fn indexes_global_and_local_agents_and_skills_in_deterministic_order() {
        let root = temp_dir();
        let home = root.join("home/.freehand");
        let project = root.join("repo");
        let nested = project.join("crates/app");
        fs::create_dir_all(&nested).expect("nested");
        write(&project.join("Cargo.toml"), "[workspace]\n");
        write(&home.join("AGENTS.md"), "global instructions\n");
        write(&project.join("AGENTS.md"), "project instructions\n");
        write(&nested.join("AGENTS.md"), "nested instructions\n");
        write(
            &home.join("skills/global-skill/SKILL.md"),
            "---\nname: global-skill\ndescription: global skill\n---\nBody\n",
        );
        write(
            &project.join(".agents/skills/project-skill/SKILL.md"),
            "---\nname: project-skill\ndescription: project skill\n---\nBody\n",
        );
        write(
            &nested.join(".agents/skills/nested-skill/SKILL.md"),
            "---\nname: nested-skill\ndescription: nested skill\n---\nBody\n",
        );

        let manifest = compile_instruction_capability_manifest(
            InstructionCapabilityCompileInput::new(&home, &nested),
        )
        .expect("manifest");

        assert_eq!(manifest.errors, Vec::new());
        assert_eq!(
            manifest
                .agents
                .iter()
                .map(|entry| (entry.scope, entry.precedence))
                .collect::<Vec<_>>(),
            vec![
                (InstructionCapabilityScope::Global, 0),
                (InstructionCapabilityScope::Local, 10),
                (InstructionCapabilityScope::Local, 12),
            ]
        );
        assert_eq!(
            manifest
                .skills
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["global-skill", "project-skill", "nested-skill"]
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn records_bad_skill_frontmatter_without_dropping_valid_entries() {
        let root = temp_dir();
        let home = root.join("home/.freehand");
        let cwd = root.join("repo");
        fs::create_dir_all(&cwd).expect("cwd");
        write(&cwd.join("Cargo.toml"), "[workspace]\n");
        write(&home.join("AGENTS.md"), "global instructions\n");
        write(
            &home.join("skills/good/SKILL.md"),
            "---\nname: good\ndescription: good skill\n---\nBody\n",
        );
        write(&home.join("skills/bad/SKILL.md"), "no frontmatter\n");

        let manifest = compile_instruction_capability_manifest(
            InstructionCapabilityCompileInput::new(&home, &cwd),
        )
        .expect("manifest");

        assert_eq!(manifest.skills.len(), 1);
        assert_eq!(manifest.skills[0].name, "good");
        assert_eq!(manifest.errors.len(), 1);
        assert!(
            manifest.errors[0]
                .message
                .contains("missing YAML frontmatter")
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn writes_manifest_json_to_state_path() {
        let root = temp_dir();
        let home = root.join("home/.freehand");
        let cwd = root.join("repo");
        fs::create_dir_all(&cwd).expect("cwd");
        write(&cwd.join("Cargo.toml"), "[workspace]\n");
        write(&home.join("AGENTS.md"), "global instructions\n");
        let manifest = compile_instruction_capability_manifest(
            InstructionCapabilityCompileInput::new(&home, &cwd),
        )
        .expect("manifest");
        let manifest_path = home.join("state/instructions/capability-manifest.json");

        write_instruction_capability_manifest(&manifest, &manifest_path).expect("write manifest");
        let raw = fs::read_to_string(&manifest_path).expect("read manifest");
        assert!(raw.contains("\"schema_version\": 1"));
        assert!(raw.contains("freehand-instructions"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn renders_manifest_entries_as_instruction_capability_context() {
        let root = temp_dir();
        let home = root.join("home/.freehand");
        let cwd = root.join("repo");
        fs::create_dir_all(&cwd).expect("cwd");
        write(&cwd.join("Cargo.toml"), "[workspace]\n");
        write(&home.join("AGENTS.md"), "global sentinel FH-INST-GLOBAL\n");
        write(&cwd.join("AGENTS.md"), "local sentinel FH-INST-LOCAL\n");
        write(
            &home.join("skills/good/SKILL.md"),
            "---\nname: good\ndescription: good skill sentinel\n---\nSkill body sentinel FH-INST-SKILL\n",
        );
        let manifest = compile_instruction_capability_manifest(
            InstructionCapabilityCompileInput::new(&home, &cwd),
        )
        .expect("manifest");

        let context = render_instruction_capability_context(&manifest).expect("context");

        assert!(context.contains("<freehand_instruction_capability>"));
        assert!(context.contains("FH-INST-GLOBAL"));
        assert!(context.contains("FH-INST-LOCAL"));
        assert!(context.contains("good skill sentinel"));
        assert!(context.contains("FH-INST-SKILL"));
        assert!(context.contains(&manifest.manifest_fingerprint));

        fs::remove_dir_all(root).expect("cleanup");
    }
}
