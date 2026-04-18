use std::fs;

use dco3::nodes::models::NodeType;
use rmcp::model::ErrorCode;
use rmcp::serde_json::json;

use crate::mcp::nodes::{
    NodesCopyResponse, NodesDownloadResponse, NodesListResponse, NodesMkdirResponse,
    NodesReadResponse, NodesUploadResponse,
};

use super::harness::{assert_mcp_error, node, user_data, McpE2eHarness, MockMcpBackend};

#[tokio::test]
async fn nodes_list_returns_children_for_root_path() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        10,
        "TEST-OS",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    backend.insert_node(node(
        11,
        "TEST-OS-Crypto",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        Some(true),
    ));
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: NodesListResponse = harness
        .call_tool("nodes_list", json!({ "path": "/" }))
        .await;

    assert_eq!(response.total, 2);
    assert_eq!(response.items.len(), 2);

    harness.shutdown().await;
}

#[tokio::test]
async fn nodes_read_returns_structured_text_response_for_path_selector() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        10,
        "TEST-OS",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    backend.insert_node(node(
        11,
        "notes.txt",
        NodeType::File,
        Some(10),
        Some("/TEST-OS/".to_string()),
        Some("txt"),
        Some("text/plain"),
        Some(12),
        None,
    ));
    backend.set_download_payload(11, "hello world\n");
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: NodesReadResponse = harness
        .call_tool("nodes_read", json!({ "path": "/TEST-OS/notes.txt" }))
        .await;

    assert_eq!(response.resolved_file.node.id, 11);
    assert_eq!(response.encoding, "utf-8");
    assert_eq!(response.content, "hello world\n");

    harness.shutdown().await;
}

#[tokio::test]
async fn nodes_read_rejects_encrypted_file_when_secret_is_missing() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        20,
        "TEST-OS-Crypto",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        Some(true),
    ));
    backend.insert_node(node(
        21,
        "secret.txt",
        NodeType::File,
        Some(20),
        Some("/TEST-OS-Crypto/".to_string()),
        Some("txt"),
        Some("text/plain"),
        Some(6),
        Some(true),
    ));
    let harness = McpE2eHarness::start(backend, false, false).await;

    let error = harness
        .call_tool_error(
            "nodes_read",
            json!({ "path": "/TEST-OS-Crypto/secret.txt" }),
        )
        .await;

    assert_mcp_error(
        error,
        ErrorCode::INVALID_PARAMS,
        "Encrypted node access requires a stored encryption secret or startup --encryption-password.",
    );

    harness.shutdown().await;
}

#[tokio::test]
async fn nodes_download_writes_file_into_workspace() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        30,
        "TEST-OS",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    backend.insert_node(node(
        31,
        "report.txt",
        NodeType::File,
        Some(30),
        Some("/TEST-OS/".to_string()),
        Some("txt"),
        Some("text/plain"),
        Some(7),
        None,
    ));
    backend.set_download_payload(31, "content");
    let harness = McpE2eHarness::start(backend, false, false).await;
    fs::create_dir_all(harness.workspace_path("downloads")).unwrap();

    let response: NodesDownloadResponse = harness
        .call_tool(
            "nodes_download",
            json!({
                "path": "/TEST-OS/report.txt",
                "target_path": "downloads/report.txt"
            }),
        )
        .await;

    assert_eq!(response.succeeded, 1);
    assert_eq!(
        fs::read_to_string(harness.workspace_path("downloads/report.txt")).unwrap(),
        "content"
    );

    harness.shutdown().await;
}

#[tokio::test]
async fn nodes_upload_uploads_a_file_and_returns_share_message() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        40,
        "Uploads",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    let harness = McpE2eHarness::start(backend.clone(), false, false).await;
    fs::write(harness.workspace_path("report.txt"), "payload").unwrap();

    let response: NodesUploadResponse = harness
        .call_tool(
            "nodes_upload",
            json!({
                "source_path": "report.txt",
                "parent_node_id": 40,
                "share": true
            }),
        )
        .await;

    assert_eq!(response.succeeded, 1);
    assert!(response
        .share_message
        .as_deref()
        .unwrap_or_default()
        .contains("https://example.com/public/download-shares/"));

    harness.shutdown().await;
}

#[tokio::test]
async fn nodes_upload_rejects_directory_without_recursive_flag() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        50,
        "Uploads",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    let harness = McpE2eHarness::start(backend, false, false).await;
    fs::create_dir_all(harness.workspace_path("folder")).unwrap();

    let error = harness
        .call_tool_error(
            "nodes_upload",
            json!({
                "source_path": "folder",
                "parent_node_id": 50
            }),
        )
        .await;

    assert_mcp_error(
        error,
        ErrorCode::INVALID_PARAMS,
        "Container upload requires recursive flag",
    );

    harness.shutdown().await;
}

#[tokio::test]
async fn nodes_copy_returns_copied_count_for_path_selectors() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        60,
        "Source",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    backend.insert_node(node(
        61,
        "file.txt",
        NodeType::File,
        Some(60),
        Some("/Source/".to_string()),
        Some("txt"),
        Some("text/plain"),
        Some(4),
        None,
    ));
    backend.insert_node(node(
        62,
        "Target",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: NodesCopyResponse = harness
        .call_tool(
            "nodes_copy",
            json!({
                "source_path": "/Source/file.txt",
                "target_parent_path": "/Target"
            }),
        )
        .await;

    assert_eq!(response.copied_count, 1);
    assert_eq!(response.source_node_id, 61);
    assert_eq!(response.target_parent_node_id, 62);

    harness.shutdown().await;
}

#[tokio::test]
async fn nodes_mkdir_creates_room_for_room_parent() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        70,
        "Projects",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    let admin = user_data(700, "alice");
    backend.insert_user(admin);
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: NodesMkdirResponse = harness
        .call_tool(
            "nodes_mkdir",
            json!({
                "parent_node_id": 70,
                "name": "Engineering",
                "type": "room",
                "admin_users": ["alice"]
            }),
        )
        .await;

    assert_eq!(response.parent_node_id, 70);
    assert_eq!(response.created_path, "/Projects/Engineering");

    harness.shutdown().await;
}

#[tokio::test]
async fn nodes_rm_rejects_folder_without_recursive_flag() {
    let backend = MockMcpBackend::new();
    backend.insert_node(node(
        80,
        "Projects",
        NodeType::Room,
        None,
        Some("/".to_string()),
        None,
        None,
        None,
        None,
    ));
    backend.insert_node(node(
        81,
        "Engineering",
        NodeType::Folder,
        Some(80),
        Some("/Projects/".to_string()),
        None,
        None,
        None,
        None,
    ));
    let harness = McpE2eHarness::start(backend, true, false).await;

    let error = harness
        .call_tool_error("nodes_rm", json!({ "path": "/Projects/Engineering" }))
        .await;

    assert_mcp_error(
        error,
        ErrorCode::INVALID_PARAMS,
        "Recursive delete is required for folders and rooms.",
    );

    harness.shutdown().await;
}
