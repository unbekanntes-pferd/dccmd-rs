use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    model::Meta,
    tool, tool_router, ErrorData, Peer, RoleServer,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    app::nodes::download::DownloadOutcome,
    mcp::{progress::McpProgressReporter, server::DccmdMcpServer},
};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesDownloadRequest {
    pub node_id: Option<u64>,
    pub path: Option<String>,
    pub target_path: String,
    #[serde(default)]
    pub recursive: bool,
    pub velocity: Option<u8>,
    #[serde(default)]
    pub include_rooms: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DownloadFailureDto {
    pub node_id: Option<u64>,
    pub node_name: String,
    pub target: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesDownloadResponse {
    pub requested_total: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub failures: Vec<DownloadFailureDto>,
    pub target_root: String,
    pub partial: bool,
}

impl From<DownloadOutcome> for NodesDownloadResponse {
    fn from(value: DownloadOutcome) -> Self {
        Self {
            requested_total: value.requested_total,
            succeeded: value.succeeded,
            failed: value.failed,
            failures: value
                .failures
                .into_iter()
                .map(|failure| DownloadFailureDto {
                    node_id: failure.node_id,
                    node_name: failure.node_name,
                    target: failure.target,
                    reason: failure.reason,
                })
                .collect(),
            target_root: value.target_root,
            partial: value.partial,
        }
    }
}

#[tool_router(router = generated_nodes_download_router)]
impl DccmdMcpServer {
    #[tool(name = "nodes_download")]
    pub(crate) async fn nodes_download(
        &self,
        Parameters(request): Parameters<NodesDownloadRequest>,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> Result<Json<NodesDownloadResponse>, ErrorData> {
        let progress = McpProgressReporter::from_meta(&meta, peer);
        self.platform()
            .download_nodes(request, progress)
            .await
            .map(NodesDownloadResponse::from)
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::nodes_download_tool_attr(),
        descriptions::NODES_DOWNLOAD,
    )
}
