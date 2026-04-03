use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    tool, tool_router, ErrorData,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mcp::{server::DccmdMcpServer, users::models::McpOidcIdp};

use super::{descriptions, with_description};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct UsersGetOidcIdpsRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UsersGetOidcIdpsResponse {
    pub target: String,
    pub items: Vec<McpOidcIdp>,
}

#[tool_router(router = generated_users_get_oidc_idps_router)]
impl DccmdMcpServer {
    #[tool(name = "users_get_oidc_idps")]
    pub(crate) async fn users_get_oidc_idps(
        &self,
        Parameters(_): Parameters<UsersGetOidcIdpsRequest>,
    ) -> Result<Json<UsersGetOidcIdpsResponse>, ErrorData> {
        self.platform()
            .get_oidc_idps()
            .await
            .map(Json)
            .map_err(super::super::server::map_tool_error)
    }
}

pub(super) fn tool() -> rmcp::model::Tool {
    with_description(
        DccmdMcpServer::users_get_oidc_idps_tool_attr(),
        descriptions::USERS_GET_OIDC_IDPS,
    )
}
