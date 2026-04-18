use rmcp::model::ErrorCode;
use rmcp::serde_json::json;

use crate::mcp::users::{
    UsersCreateResponse, UsersGetOidcIdpsResponse, UsersInfoResponse, UsersListResponse,
    UsersWhoamiResponse,
};

use super::harness::{assert_mcp_error, user_data, McpE2eHarness, MockMcpBackend};

#[tokio::test]
async fn users_whoami_returns_current_authenticated_user() {
    let backend = MockMcpBackend::new();
    let user = user_data(101, "octavio");
    backend.insert_user(user.clone());
    backend.set_current_user(&user);
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: UsersWhoamiResponse = harness.call_tool("users_whoami", json!({})).await;

    assert_eq!(response.user.id, 101);
    assert_eq!(response.user.username, "octavio");

    harness.shutdown().await;
}

#[tokio::test]
async fn users_get_oidc_idps_returns_structured_idp_list() {
    let backend = MockMcpBackend::new();
    backend.set_oidc_configs(vec![
        crate::app::config::api::OpenIdConfigInfo {
            id: 5,
            name: "Dracoon OpenID".to_string(),
        },
        crate::app::config::api::OpenIdConfigInfo {
            id: 39,
            name: "Azure Active Directory".to_string(),
        },
    ]);
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: UsersGetOidcIdpsResponse =
        harness.call_tool("users_get_oidc_idps", json!({})).await;

    assert_eq!(response.items.len(), 2);
    assert_eq!(response.items[0].id, 5);
    assert_eq!(response.items[1].name, "Azure Active Directory");

    harness.shutdown().await;
}

#[tokio::test]
async fn users_get_oidc_idps_maps_backend_failure_to_internal_error() {
    let backend = MockMcpBackend::new();
    backend.set_oidc_error(true);
    let harness = McpE2eHarness::start(backend, false, false).await;

    let error = harness
        .call_tool_error("users_get_oidc_idps", json!({}))
        .await;

    assert_mcp_error(
        error,
        ErrorCode::INTERNAL_ERROR,
        "Connection to DRACOON failed",
    );

    harness.shutdown().await;
}

#[tokio::test]
async fn users_list_returns_all_configured_users() {
    let backend = MockMcpBackend::new();
    backend.insert_user(user_data(110, "alice"));
    backend.insert_user(user_data(111, "bob"));
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: UsersListResponse = harness.call_tool("users_list", json!({})).await;

    assert_eq!(response.total, 2);
    assert_eq!(response.items.len(), 2);

    harness.shutdown().await;
}

#[tokio::test]
async fn users_info_rejects_ambiguous_selector() {
    let harness = McpE2eHarness::start(MockMcpBackend::new(), false, false).await;

    let error = harness
        .call_tool_error(
            "users_info",
            json!({
                "user_id": 1,
                "username": "alice"
            }),
        )
        .await;

    assert_mcp_error(
        error,
        ErrorCode::INVALID_PARAMS,
        "Provide exactly one selector: user_id or username.",
    );

    harness.shutdown().await;
}

#[tokio::test]
async fn users_create_returns_created_user() {
    let backend = MockMcpBackend::new();
    backend.set_created_user(user_data(120, "new.user"));
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: UsersCreateResponse = harness
        .call_tool(
            "users_create",
            json!({
                "first_name": "New",
                "last_name": "User",
                "email": "new.user@example.com"
            }),
        )
        .await;

    assert_eq!(response.user.id, 120);
    assert_eq!(response.user.username, "new.user");

    harness.shutdown().await;
}

#[tokio::test]
async fn users_info_returns_user_for_username_selector() {
    let backend = MockMcpBackend::new();
    backend.insert_user(user_data(130, "alice"));
    let harness = McpE2eHarness::start(backend, false, false).await;

    let response: UsersInfoResponse = harness
        .call_tool("users_info", json!({ "username": "alice" }))
        .await;

    assert_eq!(response.user.id, 130);
    assert_eq!(response.user.username, "alice");

    harness.shutdown().await;
}
