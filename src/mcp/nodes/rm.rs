use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::server::DccmdMcpServer;

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesRmRequest {
    pub node_id: Option<u64>,
    pub path: Option<String>,
    #[serde(default)]
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesRmResponse {
    pub deleted_count: u64,
    pub deleted_node_ids: Vec<u64>,
    pub source_path: String,
    pub recursive: bool,
}

#[tool_router(router = generated_nodes_rm_router)]
impl DccmdMcpServer {
    #[tool(name = "nodes_rm")]
    pub(crate) async fn nodes_rm(
        &self,
        Parameters(request): Parameters<NodesRmRequest>,
    ) -> Result<Json<NodesRmResponse>, ErrorData> {
        self.platform()
            .remove_nodes(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(DccmdMcpServer::nodes_rm_tool_attr(), descriptions::NODES_RM)
}
