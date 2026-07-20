use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathSymlinkDiagnostic {
    path: PathBuf,
    target: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathResolutionDiagnostic {
    requested: String,
    expanded: PathBuf,
    exists: bool,
    is_dir: Option<bool>,
    canonical: Option<PathBuf>,
    nearest_existing: Option<PathBuf>,
    nearest_existing_canonical: Option<PathBuf>,
    missing_suffix: Option<PathBuf>,
    symlink_ancestors: Vec<PathSymlinkDiagnostic>,
}

impl PathResolutionDiagnostic {
    fn inspect(requested: &str) -> Self {
        let expanded = expand_leading_tilde_path(requested.trim());
        let metadata = fs::metadata(&expanded).ok();
        let exists = fs::symlink_metadata(&expanded).is_ok();
        let nearest_existing = nearest_existing_path(&expanded);
        let nearest_existing_canonical = nearest_existing
            .as_ref()
            .and_then(|path| fs::canonicalize(path).ok());
        let missing_suffix = nearest_existing
            .as_ref()
            .and_then(|path| expanded.strip_prefix(path).ok())
            .filter(|suffix| !suffix.as_os_str().is_empty())
            .map(Path::to_path_buf);
        Self {
            requested: requested.to_owned(),
            canonical: fs::canonicalize(&expanded).ok(),
            symlink_ancestors: symlink_ancestors(&expanded),
            expanded,
            exists,
            is_dir: metadata.map(|metadata| metadata.is_dir()),
            nearest_existing,
            nearest_existing_canonical,
            missing_suffix,
        }
    }
    fn render(&self, label: &str) -> String {
        let mut fields = vec![
            format!("requested=`{}`", self.requested),
            format!("expanded=`{}`", self.expanded.display()),
            format!("exists={}", self.exists),
        ];
        if let Some(is_dir) = self.is_dir {
            fields.push(format!("is_dir={is_dir}"));
        }
        if let Some(canonical) = &self.canonical {
            fields.push(format!("canonical=`{}`", canonical.display()));
        }
        if let Some(nearest_existing) = &self.nearest_existing {
            fields.push(format!("nearest_existing=`{}`", nearest_existing.display()));
        }
        if let Some(nearest_existing_canonical) = &self.nearest_existing_canonical {
            fields.push(format!(
                "nearest_existing_canonical=`{}`",
                nearest_existing_canonical.display()
            ));
        }
        if let Some(missing_suffix) = &self.missing_suffix {
            fields.push(format!("missing_suffix=`{}`", missing_suffix.display()));
        }
        let symlinks = if self.symlink_ancestors.is_empty() {
            "[]".to_owned()
        } else {
            self.symlink_ancestors
                .iter()
                .map(|entry| format!("`{}` -> `{}`", entry.path.display(), entry.target.display()))
                .collect::<Vec<_>>()
                .join(", ")
        };
        fields.push(format!("symlink_ancestors=[{symlinks}]"));
        format!("{label}_path_diagnostic {}", fields.join(" "))
    }
}

pub(crate) fn path_resolution_diagnostic_text(label: &str, requested: &str) -> String {
    PathResolutionDiagnostic::inspect(requested).render(label)
}

pub(crate) fn expand_leading_tilde_path(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn nearest_existing_path(path: &Path) -> Option<PathBuf> {
    let mut candidate = path.to_path_buf();
    loop {
        if candidate.exists() {
            return Some(candidate);
        }
        if !candidate.pop() {
            return None;
        }
    }
}

fn symlink_ancestors(path: &Path) -> Vec<PathSymlinkDiagnostic> {
    let mut current = PathBuf::new();
    let mut symlinks = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            std::path::Component::RootDir => current.push(component.as_os_str()),
            std::path::Component::CurDir => current.push(component.as_os_str()),
            std::path::Component::ParentDir | std::path::Component::Normal(_) => {
                current.push(component.as_os_str());
            }
        }
        if current.as_os_str().is_empty() {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            continue;
        };
        if metadata.file_type().is_symlink()
            && let Ok(target) = fs::read_link(&current)
        {
            symlinks.push(PathSymlinkDiagnostic {
                path: current.clone(),
                target,
            });
        }
    }
    symlinks
}
