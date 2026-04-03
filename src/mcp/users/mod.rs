mod create;
mod descriptions;
mod info;
mod list;
mod models;
mod oidc_idps;
mod whoami;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::mcp::server::DccmdMcpServer;

pub use create::{UsersCreateRequest, UsersCreateResponse};
pub use info::{UsersInfoRequest, UsersInfoResponse};
pub use list::{UsersListRequest, UsersListResponse};
pub use models::McpUser;
pub use oidc_idps::UsersGetOidcIdpsResponse;
pub use whoami::UsersWhoamiResponse;

pub fn router() -> ToolRouter<DccmdMcpServer> {
    ToolRouter::new()
        .with_route((whoami::tool(), DccmdMcpServer::users_whoami))
        .with_route((oidc_idps::tool(), DccmdMcpServer::users_get_oidc_idps))
        .with_route((list::tool(), DccmdMcpServer::users_list))
        .with_route((info::tool(), DccmdMcpServer::users_info))
        .with_route((create::tool(), DccmdMcpServer::users_create))
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
        for tool in router().list_all() {
            let description = tool.description.expect("tool description missing");
            let description = description.as_ref();
            assert!(description.contains("Purpose:"));
            assert!(description.contains("Examples:"));
            assert!(description.contains("Common failures:"));
        }
    }
}
