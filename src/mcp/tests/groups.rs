use rmcp::model::ErrorCode;
use rmcp::serde_json::json;

use crate::mcp::groups::{
    GroupsAddUsersResponse, GroupsCreateResponse, GroupsGetUsersResponse, GroupsListResponse,
};

use super::harness::{
    assert_mcp_error, group, group_user, user_data, McpE2eHarness, MockMcpBackend,
};

#[tokio::test]
async fn groups_list_returns_all_groups() {
    let backend = MockMcpBackend::new();
    backend.insert_group(group(201, "Engineering", Some(2)));
    backend.insert_group(group(202, "Operations", Some(1)));
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: GroupsListResponse = harness.call_tool("groups_list", json!({})).await;

    assert_eq!(response.total, 2);
    assert_eq!(response.items.len(), 2);

    harness.shutdown().await;
}

#[tokio::test]
async fn groups_create_returns_created_group() {
    let backend = MockMcpBackend::new();
    backend.set_created_group(group(210, "Platform", Some(0)));
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: GroupsCreateResponse = harness
        .call_tool("groups_create", json!({ "name": "Platform" }))
        .await;

    assert_eq!(response.group.id, 210);
    assert_eq!(response.group.name, "Platform");

    harness.shutdown().await;
}

#[tokio::test]
async fn groups_get_users_supports_group_name_selector() {
    let backend = MockMcpBackend::new();
    backend.insert_group(group(220, "Engineering", Some(2)));
    backend.set_group_users(220, vec![group_user(1, "alice"), group_user(2, "bob")]);
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: GroupsGetUsersResponse = harness
        .call_tool("groups_get_users", json!({ "group_name": "Engineering" }))
        .await;

    assert_eq!(response.group.id, 220);
    assert_eq!(response.total, 2);

    harness.shutdown().await;
}

#[tokio::test]
async fn groups_get_users_rejects_ambiguous_selector() {
    let harness = McpE2eHarness::start(MockMcpBackend::new(), false, false).await;

    let error = harness
        .call_tool_error(
            "groups_get_users",
            json!({
                "group_id": 220,
                "group_name": "Engineering"
            }),
        )
        .await;

    assert_mcp_error(
        error,
        ErrorCode::INVALID_PARAMS,
        "Provide exactly one selector: group_id or group_name.",
    );

    harness.shutdown().await;
}

#[tokio::test]
async fn groups_add_users_resolves_usernames_and_deduplicates_ids() {
    let backend = MockMcpBackend::new();
    backend.insert_group(group(230, "Engineering", Some(0)));
    backend.insert_user(user_data(301, "alice"));
    backend.insert_user(user_data(302, "bob"));
    let harness = McpE2eHarness::start(backend.clone(), false, false).await;

    let response: GroupsAddUsersResponse = harness
        .call_tool(
            "groups_add_users",
            json!({
                "group_id": 230,
                "user_ids": [301],
                "usernames": ["alice", "bob"]
            }),
        )
        .await;

    assert_eq!(response.added_count, 2);
    assert_eq!(response.added_user_ids, vec![301, 302]);
    assert_eq!(backend.add_group_users_calls(), vec![(230, vec![301, 302])]);

    harness.shutdown().await;
}
