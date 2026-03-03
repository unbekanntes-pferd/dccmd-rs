use async_trait::async_trait;
use dco3::{
    auth::Connected,
    groups::{ChangeGroupMembersRequest, GroupUser},
    nodes::{Node, RoomGuestUserInvitation},
    users::{CreateUserRequest, UpdateUserRequest, UserData, UserItem, UsersFilter},
    Dracoon, Groups, ListAllParams, Nodes, RangedItems, Rooms, Users,
};

use crate::core::models::DcCmdError;

#[async_trait]
pub trait UsersApi: Send + Sync {
    async fn find_user_id_by_username(&self, user_name: &str) -> Result<u64, DcCmdError>;
}

#[async_trait]
impl UsersApi for Dracoon<Connected> {
    async fn find_user_id_by_username(&self, user_name: &str) -> Result<u64, DcCmdError> {
        let user_filter = UsersFilter::username_equals(user_name);
        let params = ListAllParams::builder().with_filter(user_filter).build();
        let results = self.users().get_users(Some(params), None, None).await?;

        let Some(user) = results.items.into_iter().find(|u| u.user_name == user_name) else {
            let msg = format!("No user found with username: {user_name}");
            return Err(DcCmdError::InvalidArgument(msg));
        };

        Ok(user.id)
    }
}

#[async_trait]
pub trait UsersCommandApi: Send + Sync {
    async fn create_user(&self, req: CreateUserRequest) -> Result<UserData, DcCmdError>;

    async fn get_users(
        &self,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<UserItem>, DcCmdError>;

    async fn delete_user(&self, user_id: u64) -> Result<(), DcCmdError>;

    async fn get_user(&self, user_id: u64) -> Result<UserData, DcCmdError>;

    async fn update_user(
        &self,
        user_id: u64,
        req: UpdateUserRequest,
    ) -> Result<UserData, DcCmdError>;

    async fn add_group_users(&self, group_id: u64, user_ids: Vec<u64>) -> Result<(), DcCmdError>;

    async fn get_group_users(
        &self,
        group_id: u64,
        params: Option<ListAllParams>,
    ) -> Result<RangedItems<GroupUser>, DcCmdError>;

    async fn get_node_from_path(&self, path: &str) -> Result<Option<Node>, DcCmdError>;

    async fn invite_guest_users(
        &self,
        room_id: u64,
        users: Vec<RoomGuestUserInvitation>,
    ) -> Result<(), DcCmdError>;
}

#[async_trait]
impl UsersCommandApi for Dracoon<Connected> {
    async fn create_user(&self, req: CreateUserRequest) -> Result<UserData, DcCmdError> {
        self.users().create_user(req).await.map_err(Into::into)
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

    async fn delete_user(&self, user_id: u64) -> Result<(), DcCmdError> {
        self.users().delete_user(user_id).await.map_err(Into::into)
    }

    async fn get_user(&self, user_id: u64) -> Result<UserData, DcCmdError> {
        self.users()
            .get_user(user_id, None)
            .await
            .map_err(Into::into)
    }

    async fn update_user(
        &self,
        user_id: u64,
        req: UpdateUserRequest,
    ) -> Result<UserData, DcCmdError> {
        self.users()
            .update_user(user_id, req)
            .await
            .map_err(Into::into)
    }

    async fn add_group_users(&self, group_id: u64, user_ids: Vec<u64>) -> Result<(), DcCmdError> {
        self.groups()
            .add_group_users(group_id, ChangeGroupMembersRequest::new(user_ids))
            .await
            .map(|_| ())
            .map_err(Into::into)
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

    async fn get_node_from_path(&self, path: &str) -> Result<Option<Node>, DcCmdError> {
        self.nodes()
            .get_node_from_path(path)
            .await
            .map_err(Into::into)
    }

    async fn invite_guest_users(
        &self,
        room_id: u64,
        users: Vec<RoomGuestUserInvitation>,
    ) -> Result<(), DcCmdError> {
        self.nodes()
            .invite_guest_users(room_id, users.into())
            .await
            .map_err(Into::into)
    }
}
