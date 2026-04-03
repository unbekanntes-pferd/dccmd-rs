use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{groups::models::McpGroup, server::DccmdMcpServer};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupsAddUsersRequest {
    pub group_id: Option<u64>,
    pub group_name: Option<String>,
    #[serde(default)]
    pub user_ids: Vec<u64>,
    #[serde(default)]
    pub usernames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupsAddUsersResponse {
    pub target: String,
    pub group: McpGroup,
    pub added_user_ids: Vec<u64>,
    pub added_count: u64,
}

#[tool_router(router = generated_groups_add_users_router)]
impl DccmdMcpServer {
    #[tool(name = "groups_add_users")]
    pub(crate) async fn groups_add_users(
        &self,
        Parameters(request): Parameters<GroupsAddUsersRequest>,
    ) -> Result<Json<GroupsAddUsersResponse>, ErrorData> {
        self.platform()
            .add_group_users(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::groups_add_users_tool_attr(),
        descriptions::GROUPS_ADD_USERS,
    )
}
