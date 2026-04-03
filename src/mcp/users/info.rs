use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{server::DccmdMcpServer, users::models::McpUser};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UsersInfoRequest {
    pub user_id: Option<u64>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UsersInfoResponse {
    pub target: String,
    pub user: McpUser,
}

#[tool_router(router = generated_users_info_router)]
impl DccmdMcpServer {
    #[tool(name = "users_info")]
    pub(crate) async fn users_info(
        &self,
        Parameters(request): Parameters<UsersInfoRequest>,
    ) -> Result<Json<UsersInfoResponse>, ErrorData> {
        self.platform()
            .get_user_info(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::users_info_tool_attr(),
        descriptions::USERS_INFO,
    )
}
