use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{server::DccmdMcpServer, users::models::McpUser};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UsersCreateRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub login: Option<String>,
    pub oidc_id: Option<u32>,
    #[serde(default)]
    pub mfa_enforced: bool,
    pub group_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UsersCreateResponse {
    pub target: String,
    pub user: McpUser,
    pub auth_method: String,
}

#[tool_router(router = generated_users_create_router)]
impl DccmdMcpServer {
    #[tool(name = "users_create")]
    pub(crate) async fn users_create(
        &self,
        Parameters(request): Parameters<UsersCreateRequest>,
    ) -> Result<Json<UsersCreateResponse>, ErrorData> {
        self.platform()
            .create_user(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::users_create_tool_attr(),
        descriptions::USERS_CREATE,
    )
}
