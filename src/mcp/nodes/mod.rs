mod copy;
mod descriptions;
mod download;
mod list;
mod mkdir;
mod models;
mod read;
mod rm;
mod upload;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::mcp::server::DccmdMcpServer;

pub use copy::{NodesCopyRequest, NodesCopyResponse};
pub use download::NodesDownloadRequest;
pub use list::{NodesListRequest, NodesListResponse};
pub use mkdir::{NodesMkdirRequest, NodesMkdirResponse};
pub(crate) use models::ReadableTextNode;
pub use models::{
    ListComparison, McpContainerType, NodeFilterTextOp, NodeListFilter, NodeSummary, ResolvedNode,
};
pub use read::{NodesReadRequest, NodesReadResponse};
pub use rm::{NodesRmRequest, NodesRmResponse};
pub use upload::NodesUploadRequest;

pub fn router() -> ToolRouter<DccmdMcpServer> {
    ToolRouter::new()
        .with_route((list::tool(), DccmdMcpServer::nodes_list))
        .with_route((download::tool(), DccmdMcpServer::nodes_download))
        .with_route((read::tool(), DccmdMcpServer::nodes_read))
        .with_route((upload::tool(), DccmdMcpServer::nodes_upload))
        .with_route((copy::tool(), DccmdMcpServer::nodes_copy))
        .with_route((mkdir::tool(), DccmdMcpServer::nodes_mkdir))
}

pub fn destructive_router() -> ToolRouter<DccmdMcpServer> {
    ToolRouter::new().with_route((rm::tool(), DccmdMcpServer::nodes_rm))
}

pub(super) fn with_description(
    mut tool: rmcp::model::Tool,
    description: &'static str,
) -> rmcp::model::Tool {
    tool.description = Some(description.into());
    tool
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_descriptions_include_required_contract_sections() {
        let tools = router()
            .list_all()
            .into_iter()
            .chain(destructive_router().list_all())
            .collect::<Vec<_>>();

        for tool in tools {
            let description = tool.description.expect("tool description missing");
            let description = description.as_ref();
            assert!(description.contains("Purpose:"));
            assert!(description.contains("Examples:"));
            assert!(description.contains("Common failures:"));
        }
    }
}
