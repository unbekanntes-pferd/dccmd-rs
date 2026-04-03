mod add_users;
mod create;
mod descriptions;
mod get_users;
mod list;
mod models;

use rmcp::handler::server::router::tool::ToolRouter;

use crate::mcp::server::DccmdMcpServer;

pub use add_users::{GroupsAddUsersRequest, GroupsAddUsersResponse};
pub use create::{GroupsCreateRequest, GroupsCreateResponse};
pub use get_users::{GroupsGetUsersRequest, GroupsGetUsersResponse};
pub use list::{GroupsListRequest, GroupsListResponse};
pub use models::{McpGroup, McpGroupUser};

pub fn router() -> ToolRouter<DccmdMcpServer> {
    ToolRouter::new()
        .with_route((list::tool(), DccmdMcpServer::groups_list))
        .with_route((create::tool(), DccmdMcpServer::groups_create))
        .with_route((get_users::tool(), DccmdMcpServer::groups_get_users))
        .with_route((add_users::tool(), DccmdMcpServer::groups_add_users))
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
