use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::server::DccmdMcpServer;

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesCopyRequest {
    pub source_node_id: Option<u64>,
    pub source_path: Option<String>,
    pub target_parent_node_id: Option<u64>,
    pub target_parent_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesCopyResponse {
    pub source_node_id: u64,
    pub target_parent_node_id: u64,
    pub copied_count: usize,
    pub source_path: String,
    pub target_parent_path: String,
}

#[tool_router(router = generated_nodes_copy_router)]
impl DccmdMcpServer {
    #[tool(name = "nodes_copy")]
    pub(crate) async fn nodes_copy(
        &self,
        Parameters(request): Parameters<NodesCopyRequest>,
    ) -> Result<Json<NodesCopyResponse>, ErrorData> {
        self.platform()
            .copy_nodes(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::nodes_copy_tool_attr(),
        descriptions::NODES_COPY,
    )
}
