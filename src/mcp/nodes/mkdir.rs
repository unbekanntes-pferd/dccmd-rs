use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::server::DccmdMcpServer;

use super::{descriptions, models::McpContainerType, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesMkdirRequest {
    pub parent_node_id: Option<u64>,
    pub parent_path: Option<String>,
    pub name: String,
    pub r#type: McpContainerType,
    pub classification: Option<u8>,
    pub notes: Option<String>,
    pub admin_users: Option<Vec<String>>,
    #[serde(default)]
    pub inherit_permissions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesMkdirResponse {
    pub message: String,
    pub parent_node_id: u64,
    pub created_path: String,
    pub r#type: McpContainerType,
}

#[tool_router(router = generated_nodes_mkdir_router)]
impl DccmdMcpServer {
    #[tool(name = "nodes_mkdir")]
    pub(crate) async fn nodes_mkdir(
        &self,
        Parameters(request): Parameters<NodesMkdirRequest>,
    ) -> Result<Json<NodesMkdirResponse>, ErrorData> {
        self.platform()
            .create_container(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::nodes_mkdir_tool_attr(),
        descriptions::NODES_MKDIR,
    )
}
