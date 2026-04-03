use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    model::Meta,
    tool, tool_router, ErrorData, Peer, RoleServer,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    app::nodes::upload::{UploadFailure, UploadOutcome},
    mcp::{progress::McpProgressReporter, server::DccmdMcpServer},
};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesUploadRequest {
    pub source_path: String,
    pub parent_node_id: Option<u64>,
    pub parent_path: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub keep_share_links: bool,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub skip_root: bool,
    #[serde(default)]
    pub share: bool,
    pub classification: Option<u8>,
    pub velocity: Option<u8>,
    pub share_password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UploadFailureDto {
    pub file: String,
    pub reason: String,
}

impl From<UploadFailure> for UploadFailureDto {
    fn from(value: UploadFailure) -> Self {
        Self {
            file: value.file,
            reason: value.reason,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NodesUploadResponse {
    pub requested_total: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub failures: Vec<UploadFailureDto>,
    pub target_root: String,
    pub partial: bool,
    pub share_message: Option<String>,
}

impl From<UploadOutcome> for NodesUploadResponse {
    fn from(value: UploadOutcome) -> Self {
        Self {
            requested_total: value.requested_total,
            succeeded: value.succeeded,
            failed: value.failed,
            failures: value
                .failures
                .into_iter()
                .map(UploadFailureDto::from)
                .collect(),
            target_root: value.target_root,
            partial: value.partial,
            share_message: value.share_message,
        }
    }
}

#[tool_router(router = generated_nodes_upload_router)]
impl DccmdMcpServer {
    #[tool(name = "nodes_upload")]
    pub(crate) async fn nodes_upload(
        &self,
        Parameters(request): Parameters<NodesUploadRequest>,
        meta: Meta,
        peer: Peer<RoleServer>,
    ) -> Result<Json<NodesUploadResponse>, ErrorData> {
        let progress = McpProgressReporter::from_meta(&meta, peer);
        self.platform()
            .upload_nodes(request, progress)
            .await
            .map(NodesUploadResponse::from)
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::nodes_upload_tool_attr(),
        descriptions::NODES_UPLOAD,
    )
}
