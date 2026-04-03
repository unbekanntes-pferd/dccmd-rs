use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    model::Meta,
    tool, tool_router, ErrorData, Peer, RoleServer,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{progress::McpProgressReporter, server::DccmdMcpServer};

use super::{
    descriptions,
    models::{NodeListFilter, NodeSummary, ResolvedNode},
    with_description,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesListRequest {
    pub node_id: Option<u64>,
    pub path: Option<String>,
    pub filter: Option<NodeListFilter>,
    pub offset: Option<u64>,
    pub limit: Option<u32>,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesListResponse {
    pub target: String,
    pub resolved_parent: Option<ResolvedNode>,
    pub items: Vec<NodeSummary>,
    pub total: u64,
    pub offset: u64,
    pub limit: u32,
    pub all_fetched: bool,
    pub managed: bool,
}

#[tool_router(router = generated_nodes_list_router)]
impl DccmdMcpServer {
    #[tool(name = "nodes_list")]
    pub(crate) async fn nodes_list(
        &self,
        Parameters(request): Parameters<NodesListRequest>,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> Result<Json<NodesListResponse>, ErrorData> {
        let progress = McpProgressReporter::from_meta(&meta, peer);
        self.platform()
            .list_nodes(request, progress)
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::nodes_list_tool_attr(),
        descriptions::NODES_LIST,
    )
}
