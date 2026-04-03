use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{
    groups::models::{McpGroup, McpGroupUser},
    server::DccmdMcpServer,
};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupsGetUsersRequest {
    pub group_id: Option<u64>,
    pub group_name: Option<String>,
    pub filter: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<u32>,
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupsGetUsersResponse {
    pub target: String,
    pub group: McpGroup,
    pub items: Vec<McpGroupUser>,
    pub total: u64,
    pub offset: u64,
    pub limit: u32,
    pub all_fetched: bool,
}

#[tool_router(router = generated_groups_get_users_router)]
impl DccmdMcpServer {
    #[tool(name = "groups_get_users")]
    pub(crate) async fn groups_get_users(
        &self,
        Parameters(request): Parameters<GroupsGetUsersRequest>,
    ) -> Result<Json<GroupsGetUsersResponse>, ErrorData> {
        self.platform()
            .get_group_users(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::groups_get_users_tool_attr(),
        descriptions::GROUPS_GET_USERS,
    )
}
