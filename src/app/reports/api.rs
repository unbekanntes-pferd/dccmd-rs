use async_trait::async_trait;
use dco3::{
    auth::Connected,
    eventlog::{AuditNodeList, EventlogParams, LogEventList, LogOperationList},
    public::SoftwareVersionData,
    users::UserItem,
    Dracoon, Eventlog, ListAllParams, Public, RangedItems, Users,
};

use crate::core::models::DcCmdError;

#[async_trait]
pub trait ReportsApi: Send + Sync {
    async fn get_software_version(&self) -> Result<SoftwareVersionData, DcCmdError>;

    async fn get_events(&self, params: EventlogParams) -> Result<LogEventList, DcCmdError>;

    async fn get_event_operations(&self) -> Result<LogOperationList, DcCmdError>;

    async fn get_node_permissions(
        &self,
        params: ListAllParams,
    ) -> Result<AuditNodeList, DcCmdError>;

    async fn get_users(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<UserItem>, DcCmdError>;
}

#[async_trait]
impl ReportsApi for Dracoon<Connected> {
    async fn get_software_version(&self) -> Result<SoftwareVersionData, DcCmdError> {
        self.public()
            .get_software_version()
            .await
            .map_err(Into::into)
    }

    async fn get_events(&self, params: EventlogParams) -> Result<LogEventList, DcCmdError> {
        self.eventlog().get_events(params).await.map_err(Into::into)
    }

    async fn get_event_operations(&self) -> Result<LogOperationList, DcCmdError> {
        self.eventlog()
            .get_event_operations()
            .await
            .map_err(Into::into)
    }

    #[allow(deprecated)]
    async fn get_node_permissions(
        &self,
        params: ListAllParams,
    ) -> Result<AuditNodeList, DcCmdError> {
        self.eventlog()
            .get_node_permissions(params)
            .await
            .map_err(Into::into)
    }

    async fn get_users(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<UserItem>, DcCmdError> {
        self.users()
            .get_users(params, None, None)
            .await
            .map_err(Into::into)
    }
}
