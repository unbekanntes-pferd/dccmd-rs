use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{ServerCapabilities, ServerInfo},
    tool_handler, ErrorData, ServerHandler, ServiceExt,
};

use crate::{
    core::models::DcCmdError,
    mcp::{groups, nodes, platform::McpPlatform, users},
};

#[derive(Clone)]
pub struct DccmdMcpServer {
    tool_router: ToolRouter<Self>,
    platform: McpPlatform,
    allow_destructive: bool,
}

impl DccmdMcpServer {
    pub fn new(platform: McpPlatform, allow_destructive: bool) -> Self {
        let mut tool_router = nodes::router() + users::router() + groups::router();
        if allow_destructive {
            tool_router = tool_router + nodes::destructive_router();
        }

        Self {
            tool_router,
            platform,
            allow_destructive,
        }
    }

    pub fn platform(&self) -> &McpPlatform {
        &self.platform
    }

    pub async fn serve_stdio(self) -> Result<(), DcCmdError> {
        let service = self
            .serve(rmcp::transport::io::stdio())
            .await
            .map_err(|error| {
                DcCmdError::InvalidArgument(format!("Failed to start MCP stdio server: {error}"))
            })?;

        let _ = service.waiting().await.map_err(|error| {
            DcCmdError::InvalidArgument(format!(
                "MCP stdio server terminated unexpectedly: {error}"
            ))
        })?;

        Ok(())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DccmdMcpServer {
    fn get_info(&self) -> ServerInfo {
        let destructive = if self.allow_destructive {
            "enabled"
        } else {
            "disabled"
        };
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(format!(
                "dccmd stdio MCP server bound to {}. Tools only. Resources and prompts are disabled. Destructive tools are {}.",
                self.platform.target(),
                destructive
            ))
    }
}

pub fn map_tool_error(error: DcCmdError) -> ErrorData {
    match error {
        DcCmdError::InvalidPath(message) => ErrorData::resource_not_found(message, None),
        DcCmdError::InvalidArgument(message) => ErrorData::invalid_params(message, None),
        DcCmdError::InvalidAccount => ErrorData::invalid_params(
            "No stored auth is available for the fixed MCP target. Run a normal authenticated CLI command first to store a refresh token.",
            None,
        ),
        other => ErrorData::internal_error(other.to_string(), None),
    }
}
