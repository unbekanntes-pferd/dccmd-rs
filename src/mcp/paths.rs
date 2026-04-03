use std::{
    env,
    path::{Component, Path, PathBuf},
};

use crate::core::models::DcCmdError;

#[derive(Debug, Clone)]
pub struct WorkspacePathGuard {
    root: PathBuf,
}

impl WorkspacePathGuard {
    pub fn from_current_dir() -> Result<Self, DcCmdError> {
        let cwd = env::current_dir().map_err(|_| DcCmdError::IoError)?;
        Self::new(cwd)
    }

    pub fn new(root: PathBuf) -> Result<Self, DcCmdError> {
        let root = root.canonicalize().map_err(|_| DcCmdError::IoError)?;
        Ok(Self { root })
    }

    pub fn resolve_existing_source(&self, input: &str) -> Result<PathBuf, DcCmdError> {
        let candidate = self.resolve_candidate(input)?;
        let canonical = candidate
            .canonicalize()
            .map_err(|_| DcCmdError::InvalidPath(input.to_string()))?;
        self.ensure_within_root(&canonical, input)?;
        Ok(canonical)
    }

    pub fn resolve_target_path(&self, input: &str) -> Result<PathBuf, DcCmdError> {
        let candidate = self.resolve_candidate(input)?;
        let ancestor = deepest_existing_ancestor(&candidate)
            .ok_or_else(|| DcCmdError::InvalidPath(input.to_string()))?;
        let canonical_ancestor = ancestor
            .canonicalize()
            .map_err(|_| DcCmdError::InvalidPath(input.to_string()))?;
        self.ensure_within_root(&canonical_ancestor, input)?;
        Ok(candidate)
    }

    fn resolve_candidate(&self, input: &str) -> Result<PathBuf, DcCmdError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(DcCmdError::InvalidArgument(
                "Path argument must not be empty.".to_string(),
            ));
        }

        let raw = PathBuf::from(trimmed);
        let joined = if raw.is_absolute() {
            raw
        } else {
            self.root.join(raw)
        };

        lexical_normalize(&joined).ok_or_else(|| {
            DcCmdError::InvalidPath(format!("Local path escapes workspace root: {trimmed}"))
        })
    }

    fn ensure_within_root(&self, candidate: &Path, input: &str) -> Result<(), DcCmdError> {
        if candidate.starts_with(&self.root) {
            Ok(())
        } else {
            Err(DcCmdError::InvalidPath(format!(
                "Local path escapes workspace root: {input}"
            )))
        }
    }
}

fn deepest_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }
}

fn lexical_normalize(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    let mut depth = 0usize;
    let mut has_root = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                has_root = true;
                normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR));
            }
            Component::CurDir => {}
            Component::Normal(part) => {
                normalized.push(part);
                depth += 1;
            }
            Component::ParentDir => {
                if depth == 0 {
                    return None;
                }
                normalized.pop();
                depth -= 1;
            }
        }
    }

    if normalized.as_os_str().is_empty() && has_root {
        normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR));
    }

    Some(normalized)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock drift")
            .as_nanos();
        env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    #[test]
    fn resolve_existing_source_rejects_parent_escape() {
        let root = unique_temp_dir("dccmd-mcp-guard");
        fs::create_dir_all(root.join("workspace")).unwrap();
        let guard = WorkspacePathGuard::new(root.join("workspace")).unwrap();

        let err = guard.resolve_existing_source("../secret.txt").unwrap_err();

        assert!(matches!(err, DcCmdError::InvalidPath(_)));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_target_path_accepts_in_workspace_relative_path() {
        let root = unique_temp_dir("dccmd-mcp-guard");
        fs::create_dir_all(root.join("workspace")).unwrap();
        let guard = WorkspacePathGuard::new(root.join("workspace")).unwrap();

        let resolved = guard.resolve_target_path("downloads/report.pdf").unwrap();

        assert!(resolved.starts_with(&guard.root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_target_path_rejects_absolute_path_outside_workspace() {
        let root = unique_temp_dir("dccmd-mcp-guard");
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let guard = WorkspacePathGuard::new(workspace).unwrap();
        let outside = root.join("outside").join("report.pdf");

        let err = guard
            .resolve_target_path(outside.to_string_lossy().as_ref())
            .unwrap_err();

        assert!(matches!(err, DcCmdError::InvalidPath(_)));
        let _ = fs::remove_dir_all(root);
    }
}
