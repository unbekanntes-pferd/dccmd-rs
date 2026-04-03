use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{nodes::ResolvedNode, server::DccmdMcpServer};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesReadRequest {
    pub node_id: Option<u64>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesReadResponse {
    pub target: String,
    pub resolved_file: ResolvedNode,
    pub file_type: Option<String>,
    pub media_type: Option<String>,
    pub encoding: String,
    pub size: u64,
    pub content: String,
}

#[tool_router(router = generated_nodes_read_router)]
impl DccmdMcpServer {
    #[tool(name = "nodes_read")]
    pub(crate) async fn nodes_read(
        &self,
        Parameters(request): Parameters<NodesReadRequest>,
    ) -> Result<Json<NodesReadResponse>, ErrorData> {
        self.platform()
            .read_node_content(request)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::nodes_read_tool_attr(),
        descriptions::NODES_READ,
    )
}
