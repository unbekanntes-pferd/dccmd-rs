use std::path::Path;

#[cfg(test)]
use std::{collections::HashSet, sync::Mutex};

use crate::core::models::DcCmdError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStyle {
    Unix,
    Windows,
}

pub trait Filesystem: Send + Sync {
    fn path_style(&self) -> PathStyle;

    fn create_dir_all(&self, path: &str) -> Result<(), DcCmdError>;

    #[cfg(test)]
    fn create_file(&self, path: &str) -> Result<(), DcCmdError>;

    #[cfg(test)]
    fn exists(&self, path: &str) -> bool;

    fn is_dir(&self, path: &str) -> bool;

    fn normalize(&self, path: &str) -> String {
        normalize_path_with_style(self.path_style(), path)
    }

    fn join(&self, base: &str, child: &str) -> String {
        join_paths_with_style(self.path_style(), base, child)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OSFileSystem;

impl Filesystem for OSFileSystem {
    fn path_style(&self) -> PathStyle {
        if cfg!(windows) {
            PathStyle::Windows
        } else {
            PathStyle::Unix
        }
    }

    fn create_dir_all(&self, path: &str) -> Result<(), DcCmdError> {
        std::fs::create_dir_all(path).map_err(|_| DcCmdError::IoError)
    }

    #[cfg(test)]
    fn create_file(&self, path: &str) -> Result<(), DcCmdError> {
        std::fs::File::create(path)
            .map(|_| ())
            .map_err(|_| DcCmdError::IoError)
    }

    #[cfg(test)]
    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn is_dir(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct MockFilesystem {
    style: PathStyle,
    directories: Mutex<HashSet<String>>,
    files: Mutex<HashSet<String>>,
}

#[cfg(test)]
impl MockFilesystem {
    pub fn new(style: PathStyle) -> Self {
        Self {
            style,
            directories: Mutex::new(HashSet::new()),
            files: Mutex::new(HashSet::new()),
        }
    }

    pub fn with_directories(style: PathStyle, directories: &[&str]) -> Self {
        let fs = Self::new(style);
        for dir in directories {
            let _ = fs.create_dir_all(dir);
        }
        fs
    }

    fn parent_path(&self, path: &str) -> Option<String> {
        let normalized = self.normalize(path);
        match self.path_style() {
            PathStyle::Unix => {
                let path = Path::new(&normalized);
                let parent = path.parent()?;
                let parent_str = parent.to_string_lossy().to_string();
                if parent_str.is_empty() {
                    Some("/".to_string())
                } else {
                    Some(parent_str)
                }
            }
            PathStyle::Windows => {
                let trimmed = normalized.trim_end_matches('\\');
                if trimmed.is_empty()
                    || trimmed == "\\"
                    || trimmed == "\\\\"
                    || is_windows_drive_root(trimmed)
                {
                    return None;
                }

                let idx = trimmed.rfind('\\')?;
                let mut parent = trimmed[..idx].to_string();
                if parent.is_empty() {
                    parent = "\\".to_string();
                } else if parent.len() == 2 && parent.as_bytes()[1] == b':' {
                    parent.push('\\');
                }

                Some(parent)
            }
        }
    }
}

#[cfg(test)]
impl Filesystem for MockFilesystem {
    fn path_style(&self) -> PathStyle {
        self.style
    }

    fn create_dir_all(&self, path: &str) -> Result<(), DcCmdError> {
        let normalized = self.normalize(path);
        self.directories
            .lock()
            .map_err(|_| DcCmdError::IoError)?
            .insert(normalized);
        Ok(())
    }

    #[cfg(test)]
    fn create_file(&self, path: &str) -> Result<(), DcCmdError> {
        if let Some(parent) = self.parent_path(path) {
            self.create_dir_all(&parent)?;
        }

        let normalized = self.normalize(path);
        self.files
            .lock()
            .map_err(|_| DcCmdError::IoError)?
            .insert(normalized);
        Ok(())
    }

    #[cfg(test)]
    fn exists(&self, path: &str) -> bool {
        let normalized = self.normalize(path);
        self.directories
            .lock()
            .map(|dirs| dirs.contains(&normalized))
            .unwrap_or(false)
            || self
                .files
                .lock()
                .map(|files| files.contains(&normalized))
                .unwrap_or(false)
    }

    fn is_dir(&self, path: &str) -> bool {
        let normalized = self.normalize(path);
        self.directories
            .lock()
            .map(|dirs| dirs.contains(&normalized))
            .unwrap_or(false)
    }
}

pub fn join_paths_with_style(style: PathStyle, base: &str, child: &str) -> String {
    let normalized_base = normalize_path_with_style(style, base);
    let normalized_child = normalize_path_with_style(style, child);

    if normalized_child.is_empty() {
        return normalized_base;
    }

    if is_absolute_path(style, &normalized_child) {
        return normalized_child;
    }

    if normalized_base.is_empty() {
        return normalized_child;
    }

    let separator = separator(style);
    let mut base = normalized_base.trim_end_matches(separator).to_string();

    if base == separator.to_string() {
        base.push_str(normalized_child.trim_start_matches(separator));
        return base;
    }

    if is_windows_drive_root(&base) {
        return format!("{base}\\{}", normalized_child.trim_start_matches('\\'));
    }

    format!(
        "{base}{separator}{}",
        normalized_child.trim_start_matches(separator)
    )
}

pub fn normalize_path_with_style(style: PathStyle, path: &str) -> String {
    match style {
        PathStyle::Unix => normalize_unix_path(path),
        PathStyle::Windows => normalize_windows_path(path),
    }
}

fn separator(style: PathStyle) -> char {
    match style {
        PathStyle::Unix => '/',
        PathStyle::Windows => '\\',
    }
}

fn is_windows_drive_root(path: &str) -> bool {
    path.len() == 3
        && path.as_bytes()[1] == b':'
        && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/')
}

fn is_absolute_path(style: PathStyle, path: &str) -> bool {
    match style {
        PathStyle::Unix => path.starts_with('/'),
        PathStyle::Windows => {
            path.starts_with("\\")
                || path.starts_with("//")
                || (path.len() >= 3
                    && path.as_bytes()[1] == b':'
                    && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/'))
        }
    }
}

fn normalize_unix_path(path: &str) -> String {
    let raw = path.replace('\\', "/");
    let absolute = raw.starts_with('/');
    let mut stack: Vec<&str> = Vec::new();

    for segment in raw.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                let _ = stack.pop();
            }
            _ => stack.push(segment),
        }
    }

    let joined = stack.join("/");

    if absolute {
        if joined.is_empty() {
            "/".to_string()
        } else {
            format!("/{joined}")
        }
    } else {
        joined
    }
}

fn normalize_windows_path(path: &str) -> String {
    let raw = path.replace('/', "\\");
    if raw.is_empty() {
        return String::new();
    }

    let mut prefix = String::new();
    let mut rest = raw.as_str();
    let mut absolute_after_prefix = false;

    if let Some(stripped) = raw.strip_prefix("\\\\") {
        prefix = "\\\\".to_string();
        rest = stripped;
        absolute_after_prefix = true;
    } else if raw.len() >= 2 && raw.as_bytes()[1] == b':' {
        prefix = raw[..2].to_string();
        rest = &raw[2..];
        absolute_after_prefix = rest.starts_with('\\');
    } else if raw.starts_with('\\') {
        prefix = "\\".to_string();
        rest = raw.trim_start_matches('\\');
        absolute_after_prefix = true;
    }

    let mut stack: Vec<&str> = Vec::new();

    for segment in rest.split('\\') {
        match segment {
            "" | "." => {}
            ".." => {
                let _ = stack.pop();
            }
            _ => stack.push(segment),
        }
    }

    let joined = stack.join("\\");

    if prefix == "\\\\" {
        if joined.is_empty() {
            "\\\\".to_string()
        } else {
            format!("\\\\{joined}")
        }
    } else if !prefix.is_empty() {
        if joined.is_empty() {
            if absolute_after_prefix {
                format!("{prefix}\\")
            } else {
                prefix
            }
        } else {
            format!("{prefix}\\{joined}")
        }
    } else if absolute_after_prefix {
        if joined.is_empty() {
            "\\".to_string()
        } else {
            format!("\\{joined}")
        }
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::{
        join_paths_with_style, normalize_path_with_style, Filesystem, MockFilesystem, PathStyle,
    };

    #[test]
    fn test_normalize_unix_path() {
        assert_eq!(
            normalize_path_with_style(PathStyle::Unix, "/tmp//a\\b/./c"),
            "/tmp/a/b/c"
        );
    }

    #[test]
    fn test_normalize_windows_path() {
        assert_eq!(
            normalize_path_with_style(PathStyle::Windows, "C:/tmp//a\\b/./c"),
            r"C:\tmp\a\b\c"
        );
    }

    #[test]
    fn test_join_paths_windows() {
        assert_eq!(
            join_paths_with_style(PathStyle::Windows, r"C:\tmp", "sub/file.txt"),
            r"C:\tmp\sub\file.txt"
        );
    }

    #[test]
    fn test_join_paths_unix() {
        assert_eq!(
            join_paths_with_style(PathStyle::Unix, "/tmp", "sub\\file.txt"),
            "/tmp/sub/file.txt"
        );
    }

    #[test]
    fn test_join_paths_windows_drive_root() {
        assert_eq!(
            join_paths_with_style(PathStyle::Windows, r"C:\", "tmp/file.txt"),
            r"C:\tmp\file.txt"
        );
    }

    #[test]
    fn test_join_paths_windows_absolute_child_stays_absolute() {
        assert_eq!(
            join_paths_with_style(PathStyle::Windows, r"C:\tmp", r"D:\target\file.txt"),
            r"D:\target\file.txt"
        );
    }

    #[test]
    fn test_mock_filesystem_create_file_adds_parent_dirs_unix() {
        let fs = MockFilesystem::new(PathStyle::Unix);
        fs.create_file("/tmp/a/b/report.txt").unwrap();

        assert!(fs.exists("/tmp/a/b/report.txt"));
        assert!(fs.is_dir("/tmp/a/b"));
    }

    #[test]
    fn test_mock_filesystem_create_file_adds_parent_dirs_windows() {
        let fs = MockFilesystem::new(PathStyle::Windows);
        fs.create_file("C:/tmp/a/b/report.txt").unwrap();

        assert!(fs.exists(r"C:\tmp\a\b\report.txt"));
        assert!(fs.is_dir(r"C:\tmp\a\b"));
        assert!(!fs.is_dir(r"C:\tmp\a\b\report.txt"));
    }
}
