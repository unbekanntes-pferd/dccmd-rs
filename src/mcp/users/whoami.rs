use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{server::DccmdMcpServer, users::models::McpUser};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct UsersWhoamiRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UsersWhoamiResponse {
    pub target: String,
    pub user: McpUser,
}

#[tool_router(router = generated_users_whoami_router)]
impl DccmdMcpServer {
    #[tool(name = "users_whoami")]
    pub(crate) async fn users_whoami(
        &self,
        Parameters(_): Parameters<UsersWhoamiRequest>,
    ) -> Result<Json<UsersWhoamiResponse>, ErrorData> {
        self.platform()
            .get_current_user_info()
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::users_whoami_tool_attr(),
        descriptions::USERS_WHOAMI,
    )
}
