use async_trait::async_trait;
use dco3::{
    auth::Connected,
    groups::{ChangeGroupMembersRequest, CreateGroupRequest, Group, GroupUser},
    Dracoon, Groups, ListAllParams, RangedItems,
};

use crate::core::models::DcCmdError;

#[async_trait]
pub trait GroupsApi: Send + Sync {
    async fn get_groups(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<Group>, DcCmdError>;

    async fn get_group_users(
        &self,
        group_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<GroupUser>, DcCmdError>;

    async fn create_group(&self, name: &str) -> Result<Group, DcCmdError>;

    async fn delete_group(&self, group_id: u64) -> Result<(), DcCmdError>;

    async fn add_group_users(&self, group_id: u64, user_ids: Vec<u64>) -> Result<(), DcCmdError>;
}

#[async_trait]
impl GroupsApi for Dracoon<Connected> {
    async fn get_groups(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<Group>, DcCmdError> {
        self.groups().get_groups(params).await.map_err(Into::into)
    }

    async fn get_group_users(
        &self,
        group_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<GroupUser>, DcCmdError> {
        self.groups()
            .get_group_users(group_id, params)
            .await
            .map_err(Into::into)
    }

    async fn create_group(&self, name: &str) -> Result<Group, DcCmdError> {
        let req = CreateGroupRequest::new(name.to_string(), None);
        self.groups().create_group(req).await.map_err(Into::into)
    }

    async fn delete_group(&self, group_id: u64) -> Result<(), DcCmdError> {
        self.groups()
            .delete_group(group_id)
            .await
            .map_err(Into::into)
    }

    async fn add_group_users(&self, group_id: u64, user_ids: Vec<u64>) -> Result<(), DcCmdError> {
        let req = ChangeGroupMembersRequest::new(user_ids);
        self.groups()
            .add_group_users(group_id, req)
            .await
            .map(|_| ())
            .map_err(Into::into)
    }
}
