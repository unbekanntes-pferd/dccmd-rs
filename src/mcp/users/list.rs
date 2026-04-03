use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{server::DccmdMcpServer, users::models::McpUser};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UsersListRequest {
    pub filter: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<u32>,
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UsersListResponse {
    pub target: String,
    pub items: Vec<McpUser>,
    pub total: u64,
    pub offset: u64,
    pub limit: u32,
    pub all_fetched: bool,
}

#[tool_router(router = generated_users_list_router)]
impl DccmdMcpServer {
    #[tool(name = "users_list")]
    pub(crate) async fn users_list(
        &self,
        Parameters(request): Parameters<UsersListRequest>,
    ) -> Result<Json<UsersListResponse>, ErrorData> {
        self.platform()
            .list_users(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::users_list_tool_attr(),
        descriptions::USERS_LIST,
    )
}
