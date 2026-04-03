use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{groups::models::McpGroup, server::DccmdMcpServer};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupsListRequest {
    pub filter: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<u32>,
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupsListResponse {
    pub target: String,
    pub items: Vec<McpGroup>,
    pub total: u64,
    pub offset: u64,
    pub limit: u32,
    pub all_fetched: bool,
}

#[tool_router(router = generated_groups_list_router)]
impl DccmdMcpServer {
    #[tool(name = "groups_list")]
    pub(crate) async fn groups_list(
        &self,
        Parameters(request): Parameters<GroupsListRequest>,
    ) -> Result<Json<GroupsListResponse>, ErrorData> {
        self.platform()
            .list_groups(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::groups_list_tool_attr(),
        descriptions::GROUPS_LIST,
    )
}
