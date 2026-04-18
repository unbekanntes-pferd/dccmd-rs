mod groups;
mod harness;
mod nodes;
mod users;

use harness::{McpE2eHarness, MockMcpBackend};

#[tokio::test]
async fn safe_server_lists_all_non_destructive_tools() {
    let harness = McpE2eHarness::start(MockMcpBackend::new(), false, false).await;

    let tool_names = harness.list_tool_names().await;

    assert_eq!(
        tool_names,
        vec![
            "groups_add_users",
            "groups_create",
            "groups_get_users",
            "groups_list",
            "nodes_copy",
            "nodes_download",
            "nodes_list",
            "nodes_mkdir",
            "nodes_read",
            "nodes_upload",
            "users_create",
            "users_get_oidc_idps",
            "users_info",
            "users_list",
            "users_whoami",
        ]
    );

    harness.shutdown().await;
}

#[tokio::test]
async fn destructive_server_includes_nodes_rm_tool() {
    let harness = McpE2eHarness::start(MockMcpBackend::new(), true, false).await;

    let tool_names = harness.list_tool_names().await;

    assert!(tool_names.iter().any(|name| name == "nodes_rm"));

    harness.shutdown().await;
}
