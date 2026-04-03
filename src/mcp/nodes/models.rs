use dco3::nodes::{models::NodeType, Node};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::models::DcCmdError;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodeSummary {
    pub id: u64,
    pub name: String,
    pub node_type: NodeType,
    pub parent_path: Option<String>,
    pub size: Option<u64>,
    pub is_encrypted: bool,
    pub updated_at: Option<String>,
}

impl From<&Node> for NodeSummary {
    fn from(value: &Node) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            node_type: value.node_type.clone(),
            parent_path: value.parent_path.clone(),
            size: value.size,
            is_encrypted: value.is_encrypted.unwrap_or(false),
            updated_at: value
                .timestamp_modification
                .as_ref()
                .map(chrono::DateTime::to_rfc3339),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedNode {
    pub path: String,
    #[serde(flatten)]
    pub node: NodeSummary,
}

impl ResolvedNode {
    pub fn from_node(node: &Node) -> Result<Self, DcCmdError> {
        let parent_path = node.parent_path.clone().ok_or_else(|| {
            DcCmdError::InvalidPath(format!("Node '{}' has no parent path.", node.name))
        })?;
        let path = if parent_path == "/" {
            format!("/{}/", node.name)
        } else {
            format!("{parent_path}{}/", node.name)
        };

        Ok(Self {
            path,
            node: NodeSummary::from(node),
        })
    }
}

pub(crate) trait ReadableTextNode {
    fn ensure_readable_text(&self) -> Result<(), DcCmdError>;
}

struct TextReadPolicy;

impl TextReadPolicy {
    const TEXT_EXTENSIONS: &[&str] = &[
        "txt", "md", "markdown", "json", "jsonl", "yaml", "yml", "toml", "xml", "csv", "tsv",
        "log", "ini", "conf", "cfg", "env", "py", "rs", "js", "ts", "jsx", "tsx", "java", "kt",
        "go", "sh", "bash", "zsh", "sql", "html", "css", "scss", "svg",
    ];

    const TEXT_MEDIA_TYPES: &[&str] = &[
        "application/json",
        "application/ld+json",
        "application/xml",
        "application/x-yaml",
        "application/yaml",
        "application/toml",
        "application/javascript",
        "application/x-javascript",
        "application/sql",
        "image/svg+xml",
    ];

    const GENERIC_MEDIA_TYPES: &[&str] = &[
        "application/octet-stream",
        "binary/octet-stream",
        "application/binary",
        "application/x-binary",
        "application/x-download",
        "application/unknown",
    ];

    fn media_type_decision(media_type: &str) -> PolicyDecision {
        let normalized = media_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if normalized.is_empty() {
            return PolicyDecision::Fallback;
        }
        if normalized.starts_with("text/") || Self::TEXT_MEDIA_TYPES.contains(&normalized.as_str())
        {
            return PolicyDecision::Allow;
        }
        if Self::GENERIC_MEDIA_TYPES.contains(&normalized.as_str()) {
            return PolicyDecision::Fallback;
        }
        PolicyDecision::Reject
    }

    fn file_type_allows_text(file_type: &str) -> bool {
        Self::normalize_extension(file_type)
            .map(|value| Self::TEXT_EXTENSIONS.contains(&value.as_str()))
            .unwrap_or(false)
    }

    fn name_allows_text(name: &str) -> bool {
        let extension = name
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or_default();
        Self::file_type_allows_text(extension)
    }

    fn normalize_extension(value: &str) -> Option<String> {
        let normalized = value.trim().trim_start_matches('.').to_ascii_lowercase();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    }
}

enum PolicyDecision {
    Allow,
    Fallback,
    Reject,
}

impl ReadableTextNode for Node {
    fn ensure_readable_text(&self) -> Result<(), DcCmdError> {
        match self
            .media_type
            .as_deref()
            .map(TextReadPolicy::media_type_decision)
        {
            Some(PolicyDecision::Allow) => return Ok(()),
            Some(PolicyDecision::Reject) => {
                return Err(DcCmdError::InvalidArgument(
                    "nodes_read only supports recognized text files.".to_string(),
                ));
            }
            _ => {}
        }

        if self
            .file_type
            .as_deref()
            .is_some_and(TextReadPolicy::file_type_allows_text)
            || TextReadPolicy::name_allows_text(&self.name)
        {
            Ok(())
        } else {
            Err(DcCmdError::InvalidArgument(
                "nodes_read only supports recognized text files.".to_string(),
            ))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeFilterTextOp {
    Equals,
    Contains,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListComparison {
    Gte,
    Lte,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "field", rename_all = "snake_case")]
pub enum NodeListFilter {
    Name { op: NodeFilterTextOp, value: String },
    Type { value: Vec<NodeType> },
    Encrypted { value: bool },
    BranchVersion { op: ListComparison, value: u64 },
    CreatedAt { op: ListComparison, value: String },
    UpdatedAt { op: ListComparison, value: String },
    ReferenceId { value: u64 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpContainerType {
    Folder,
    Room,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_node(name: &str, file_type: Option<&str>, media_type: Option<&str>) -> Node {
        Node {
            id: 7,
            reference_id: None,
            node_type: NodeType::File,
            name: name.to_string(),
            timestamp_creation: None,
            timestamp_modification: None,
            parent_id: None,
            parent_path: Some("/docs/".to_string()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            expire_at: None,
            hash: None,
            file_type: file_type.map(str::to_string),
            media_type: media_type.map(str::to_string),
            size: Some(42),
            classification: None,
            notes: None,
            permissions: None,
            inherit_permissions: None,
            is_encrypted: None,
            encryption_info: None,
            cnt_deleted_versions: None,
            cnt_comments: None,
            cnt_upload_shares: None,
            cnt_download_shares: None,
            recycle_bin_retention_period: None,
            has_activities_log: None,
            quota: None,
            is_favorite: None,
            branch_version: None,
            media_token: None,
            is_browsable: None,
            cnt_rooms: None,
            cnt_folders: None,
            cnt_files: None,
            auth_parent_id: None,
        }
    }

    #[test]
    fn readable_text_node_accepts_text_media_type() {
        let node = file_node("notes.bin", None, Some("text/plain; charset=utf-8"));

        assert!(node.ensure_readable_text().is_ok());
    }

    #[test]
    fn readable_text_node_accepts_allowlisted_structured_media_type() {
        let node = file_node("payload.bin", None, Some("application/json"));

        assert!(node.ensure_readable_text().is_ok());
    }

    #[test]
    fn readable_text_node_accepts_allowlisted_file_type() {
        let node = file_node("payload", Some("toml"), Some("application/octet-stream"));

        assert!(node.ensure_readable_text().is_ok());
    }

    #[test]
    fn readable_text_node_accepts_allowlisted_filename_extension() {
        let node = file_node("script.py", None, None);

        assert!(node.ensure_readable_text().is_ok());
    }

    #[test]
    fn readable_text_node_rejects_unknown_binary_file() {
        let node = file_node("archive.bin", None, Some("application/octet-stream"));

        let err = node.ensure_readable_text().unwrap_err();

        assert_eq!(
            err,
            DcCmdError::InvalidArgument(
                "nodes_read only supports recognized text files.".to_string()
            )
        );
    }

    #[test]
    fn readable_text_node_rejects_explicit_binary_media_type() {
        let node = file_node("image.json", None, Some("image/png"));

        let err = node.ensure_readable_text().unwrap_err();

        assert_eq!(
            err,
            DcCmdError::InvalidArgument(
                "nodes_read only supports recognized text files.".to_string()
            )
        );
    }
}
