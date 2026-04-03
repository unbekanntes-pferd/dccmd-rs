use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{groups::models::McpGroup, server::DccmdMcpServer};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupsCreateRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupsCreateResponse {
    pub target: String,
    pub group: McpGroup,
}

#[tool_router(router = generated_groups_create_router)]
impl DccmdMcpServer {
    #[tool(name = "groups_create")]
    pub(crate) async fn groups_create(
        &self,
        Parameters(request): Parameters<GroupsCreateRequest>,
    ) -> Result<Json<GroupsCreateResponse>, ErrorData> {
        self.platform()
            .create_group(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::groups_create_tool_attr(),
        descriptions::GROUPS_CREATE,
    )
}
