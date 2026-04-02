use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Cursor,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use dco3::{
    groups::GroupUser,
    nodes::{
        models::{NodeType, UserInfo as NodeUserInfo, UserType},
        Node, RoomGuestUserInvitation,
    },
    users::{CreateUserRequest, UpdateUserRequest, UserData, UserItem},
    ListAllParams, RangedItems,
};

use crate::{
    app::users::api::UsersCommandApi,
    core::models::{DcCmdError, ListOptions},
};

use super::{
    models::{UserImport, UsersSwitchAuthOptions},
    UsersService,
};

struct MockUsersApi {
    create_user_results: Mutex<VecDeque<Result<UserData, DcCmdError>>>,
    users_pages: Mutex<VecDeque<RangedItems<UserItem>>>,
    group_user_pages: Mutex<VecDeque<RangedItems<GroupUser>>>,
    users_by_id: HashMap<u64, UserData>,
    deleted_users: Mutex<Vec<u64>>,
    updated_users: Mutex<Vec<u64>>,
    failed_updates: HashSet<u64>,
    invite_calls: Mutex<Vec<(u64, usize)>>,
    nodes_by_path: HashMap<String, Node>,
}

impl MockUsersApi {
    fn new(
        pages: Vec<RangedItems<UserItem>>,
        nodes_by_path: HashMap<String, Node>,
        users_by_id: HashMap<u64, UserData>,
    ) -> Self {
        Self {
            create_user_results: Mutex::new(VecDeque::new()),
            users_pages: Mutex::new(VecDeque::from(pages)),
            group_user_pages: Mutex::new(VecDeque::new()),
            users_by_id,
            deleted_users: Mutex::new(Vec::new()),
            updated_users: Mutex::new(Vec::new()),
            failed_updates: HashSet::new(),
            invite_calls: Mutex::new(Vec::new()),
            nodes_by_path,
        }
    }

    fn with_group_user_pages(self, pages: Vec<RangedItems<GroupUser>>) -> Self {
        *self.group_user_pages.lock().expect("lock poisoned") = VecDeque::from(pages);
        self
    }

    fn with_failed_updates(mut self, failed_updates: HashSet<u64>) -> Self {
        self.failed_updates = failed_updates;
        self
    }

    fn with_create_user_results(self, results: Vec<Result<UserData, DcCmdError>>) -> Self {
        *self.create_user_results.lock().expect("lock poisoned") = VecDeque::from(results);
        self
    }
}

#[async_trait]
impl UsersCommandApi for Arc<MockUsersApi> {
    async fn create_user(
        &self,
        _req: CreateUserRequest,
    ) -> Result<UserData, crate::core::models::DcCmdError> {
        let mut results = self.create_user_results.lock().expect("lock poisoned");
        if let Some(result) = results.pop_front() {
            return result;
        }
        Ok(user_data_basic(99, "created.user"))
    }

    async fn get_users(
        &self,
        _params: Option<ListAllParams>,
    ) -> Result<RangedItems<UserItem>, crate::core::models::DcCmdError> {
        let mut users_pages = self.users_pages.lock().expect("lock poisoned");
        if users_pages.len() > 1 {
            Ok(users_pages.pop_front().expect("at least one page"))
        } else {
            Ok(users_pages.front().cloned().unwrap_or_else(empty_users))
        }
    }

    async fn delete_user(&self, user_id: u64) -> Result<(), crate::core::models::DcCmdError> {
        self.deleted_users
            .lock()
            .expect("lock poisoned")
            .push(user_id);
        Ok(())
    }

    async fn get_user(&self, user_id: u64) -> Result<UserData, crate::core::models::DcCmdError> {
        Ok(self
            .users_by_id
            .get(&user_id)
            .cloned()
            .unwrap_or_else(|| user_data_basic(user_id, &format!("user-{user_id}"))))
    }

    async fn update_user(
        &self,
        user_id: u64,
        _req: UpdateUserRequest,
    ) -> Result<UserData, crate::core::models::DcCmdError> {
        self.updated_users
            .lock()
            .expect("lock poisoned")
            .push(user_id);

        if self.failed_updates.contains(&user_id) {
            return Err(DcCmdError::InvalidArgument("update failed".to_string()));
        }

        Ok(self
            .users_by_id
            .get(&user_id)
            .cloned()
            .unwrap_or_else(|| user_data_basic(user_id, &format!("user-{user_id}"))))
    }

    async fn add_group_users(
        &self,
        _group_id: u64,
        _user_ids: Vec<u64>,
    ) -> Result<(), crate::core::models::DcCmdError> {
        Ok(())
    }

    async fn get_group_users(
        &self,
        _group_id: u64,
        _params: Option<ListAllParams>,
    ) -> Result<RangedItems<GroupUser>, crate::core::models::DcCmdError> {
        let mut group_user_pages = self.group_user_pages.lock().expect("lock poisoned");
        if group_user_pages.len() > 1 {
            Ok(group_user_pages.pop_front().expect("at least one page"))
        } else {
            Ok(group_user_pages
                .front()
                .cloned()
                .unwrap_or_else(empty_group_users))
        }
    }

    async fn get_node_from_path(
        &self,
        path: &str,
    ) -> Result<Option<Node>, crate::core::models::DcCmdError> {
        Ok(self.nodes_by_path.get(path).cloned())
    }

    async fn invite_guest_users(
        &self,
        room_id: u64,
        users: Vec<RoomGuestUserInvitation>,
    ) -> Result<(), crate::core::models::DcCmdError> {
        self.invite_calls
            .lock()
            .expect("lock poisoned")
            .push((room_id, users.len()));
        Ok(())
    }
}

fn users_page(items: Vec<UserItem>, total: u64) -> RangedItems<UserItem> {
    RangedItems {
        range: dco3::models::Range {
            offset: 0,
            limit: 500,
            total,
        },
        items,
    }
}

fn empty_users() -> RangedItems<UserItem> {
    users_page(Vec::new(), 0)
}

fn group_users_page(items: Vec<GroupUser>, total: u64) -> RangedItems<GroupUser> {
    RangedItems {
        range: dco3::models::Range {
            offset: 0,
            limit: 500,
            total,
        },
        items,
    }
}

fn empty_group_users() -> RangedItems<GroupUser> {
    group_users_page(Vec::new(), 0)
}

fn group_user(id: i64, user_name: &str) -> GroupUser {
    GroupUser {
        user_info: NodeUserInfo {
            id,
            user_type: UserType::Internal,
            user_name: Some(user_name.to_string()),
            first_name: None,
            last_name: None,
            email: None,
            avatar_uuid: "avatar".to_string(),
        },
        is_member: true,
    }
}

fn user_item(id: u64, user_name: &str) -> UserItem {
    UserItem {
        id,
        user_name: user_name.to_string(),
        first_name: "First".to_string(),
        last_name: "Last".to_string(),
        is_locked: false,
        avatar_uuid: "avatar".to_string(),
        email: Some(format!("{user_name}@example.com")),
        phone: None,
        expire_at: None,
        has_manageable_rooms: None,
        is_encryption_enabled: None,
        last_login_success_at: None,
        home_room_id: None,
        public_key_container: None,
        user_roles: None,
    }
}

fn user_data_basic(id: u64, user_name: &str) -> UserData {
    UserData {
        id,
        user_name: user_name.to_string(),
        first_name: "First".to_string(),
        last_name: "Last".to_string(),
        is_locked: false,
        avatar_uuid: "avatar".to_string(),
        auth_data: dco3::user::UserAuthData::new_basic(None, None),
        email: Some(format!("{user_name}@example.com")),
        phone: None,
        expire_at: None,
        has_manageable_rooms: None,
        is_encryption_enabled: None,
        last_login_success_at: None,
        home_room_id: None,
        public_key_container: None,
        user_roles: None,
        is_mfa_enabled: None,
        is_mfa_enforced: None,
    }
}

fn user_data_oidc(id: u64, user_name: &str, oidc_id: u64) -> UserData {
    let mut user = user_data_basic(id, user_name);
    user.auth_data = dco3::user::UserAuthData::new_oidc(user_name, oidc_id);
    user
}

fn user_data_ad(id: u64, user_name: &str, ad_id: u64) -> UserData {
    let mut user = user_data_basic(id, user_name);
    user.auth_data = dco3::user::UserAuthData::new_ad(user_name, ad_id);
    user
}

fn node(id: u64, node_type: NodeType, auth_parent_id: Option<u64>) -> Node {
    Node {
        id,
        reference_id: None,
        node_type,
        name: format!("node-{id}"),
        timestamp_creation: None,
        timestamp_modification: None,
        parent_id: None,
        parent_path: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
        expire_at: None,
        hash: None,
        file_type: None,
        media_type: None,
        size: None,
        classification: None,
        notes: None,
        permissions: None,
        inherit_permissions: None,
        is_encrypted: None,
        encryption_info: None,
        cnt_deleted_versions: None,
        cnt_comments: None,
        cnt_upload_shares: None,
        cnt_download_shares: None,
        recycle_bin_retention_period: None,
        has_activities_log: None,
        quota: None,
        is_favorite: None,
        branch_version: None,
        media_token: None,
        is_browsable: None,
        cnt_rooms: None,
        cnt_folders: None,
        cnt_files: None,
        auth_parent_id,
    }
}

#[tokio::test]
async fn test_list_users_collects_all_pages() {
    let users_by_id = HashMap::new();
    let mock = Arc::new(MockUsersApi::new(
        vec![
            users_page(vec![user_item(1, "u1"), user_item(2, "u2")], 501),
            users_page(vec![user_item(3, "u3")], 501),
        ],
        HashMap::new(),
        users_by_id,
    ));
    let service = UsersService::new(mock);

    let users = service
        .list_users(&ListOptions::new(None, None, None, true, false))
        .await
        .expect("users");

    assert_eq!(users.items.len(), 3);
}

#[tokio::test]
async fn test_delete_user_by_name_resolves_id_and_deletes() {
    let users_by_id = HashMap::new();
    let mock = Arc::new(MockUsersApi::new(
        vec![users_page(vec![user_item(7, "alice")], 1)],
        HashMap::new(),
        users_by_id,
    ));
    let service = UsersService::new(mock.clone());

    let message = service
        .delete_user(Some("alice".to_string()), None)
        .await
        .expect("delete result");

    assert_eq!(message, "User alice deleted");
    assert_eq!(
        mock.deleted_users.lock().expect("lock poisoned").as_slice(),
        [7]
    );
}

#[tokio::test]
async fn test_resolve_invite_room_id_for_room() {
    let mut nodes = HashMap::new();
    nodes.insert("/teams/".to_string(), node(12, NodeType::Room, None));
    let mock = Arc::new(MockUsersApi::new(
        vec![empty_users()],
        nodes,
        HashMap::new(),
    ));
    let service = UsersService::new(mock);

    let room_id = service
        .resolve_invite_room_id("https://example.com/teams/", "https://example.com")
        .await
        .expect("room id");

    assert_eq!(room_id, 12);
}

#[tokio::test]
async fn test_resolve_invite_room_id_for_folder() {
    let mut nodes = HashMap::new();
    nodes.insert(
        "/teams/sub/".to_string(),
        node(99, NodeType::Folder, Some(42)),
    );
    let mock = Arc::new(MockUsersApi::new(
        vec![empty_users()],
        nodes,
        HashMap::new(),
    ));
    let service = UsersService::new(mock);

    let room_id = service
        .resolve_invite_room_id("https://example.com/teams/sub/", "https://example.com")
        .await
        .expect("room id");

    assert_eq!(room_id, 42);
}

#[tokio::test]
async fn test_invite_user_delegates_to_api() {
    let mock = Arc::new(MockUsersApi::new(
        vec![empty_users()],
        HashMap::new(),
        HashMap::new(),
    ));
    let service = UsersService::new(mock.clone());

    service
        .invite_user(88, "A", "B", "a.b@example.com")
        .await
        .expect("invite");

    assert_eq!(
        mock.invite_calls.lock().expect("lock poisoned").as_slice(),
        [(88, 1)]
    );
}

#[tokio::test]
async fn test_delete_user_requires_identifier() {
    let mock = Arc::new(MockUsersApi::new(
        vec![empty_users()],
        HashMap::new(),
        HashMap::new(),
    ));
    let service = UsersService::new(mock);

    let err = service
        .delete_user(None, None)
        .await
        .expect_err("expected error");

    assert_eq!(
        err,
        DcCmdError::InvalidArgument("User name or user id must be provided".to_string())
    );
}

#[tokio::test]
async fn test_read_user_imports_from_csv_parses_rows() {
    let imports =
            UsersService::<Arc<MockUsersApi>>::parse_user_imports_from_reader(Cursor::new(
            "first_name,last_name,email,login,mfa_enabled\nAlice,Doe,alice@example.com,alice,true\nBob,Smith,bob@example.com,,false\n",
            ))
            .expect("imports");

    assert_eq!(imports.len(), 2);
    assert_eq!(imports[0].first_name, "Alice");
    assert_eq!(imports[0].login.as_deref(), Some("alice"));
    assert_eq!(imports[1].first_name, "Bob");
    assert_eq!(imports[1].login, None);
}

#[tokio::test]
async fn test_import_users_counts_success_and_failures() {
    let mock = Arc::new(
        MockUsersApi::new(vec![empty_users()], HashMap::new(), HashMap::new())
            .with_create_user_results(vec![
                Ok(user_data_basic(1, "u1")),
                Err(DcCmdError::InvalidArgument("create failed".to_string())),
                Ok(user_data_basic(2, "u2")),
            ]),
    );
    let service = UsersService::new(mock);

    let imports = vec![
        UserImport {
            first_name: "A".to_string(),
            last_name: "One".to_string(),
            email: "a.one@example.com".to_string(),
            login: Some("a.one".to_string()),
            mfa_enabled: Some(true),
        },
        UserImport {
            first_name: "B".to_string(),
            last_name: "Two".to_string(),
            email: "b.two@example.com".to_string(),
            login: None,
            mfa_enabled: Some(false),
        },
        UserImport {
            first_name: "C".to_string(),
            last_name: "Three".to_string(),
            email: "c.three@example.com".to_string(),
            login: None,
            mfa_enabled: None,
        },
    ];

    let mut processed = 0usize;
    let result = service
        .import_users(imports, None, |_| processed += 1)
        .await
        .expect("import result");

    assert_eq!(result.imported, 2);
    assert_eq!(result.failed, 1);
    assert_eq!(result.total, 3);
    assert_eq!(processed, 3);
}

#[tokio::test]
async fn test_switch_auth_updates_only_matching_users() {
    let mut users_by_id = HashMap::new();
    users_by_id.insert(1, user_data_oidc(1, "alice", 7));
    users_by_id.insert(2, user_data_oidc(2, "bob", 7));
    users_by_id.insert(3, user_data_basic(3, "carol"));

    let mock = Arc::new(MockUsersApi::new(
        vec![users_page(
            vec![
                user_item(1, "alice"),
                user_item(2, "bob"),
                user_item(3, "carol"),
            ],
            3,
        )],
        HashMap::new(),
        users_by_id,
    ));
    let service = UsersService::new(mock.clone());

    let opts = UsersSwitchAuthOptions::try_new(
        "oidc".to_string(),
        "local".to_string(),
        Some(7),
        None,
        None,
        None,
        None,
        Some("username".to_string()),
    )
    .expect("valid switch opts");

    let result = service.switch_auth(opts).await.expect("switch auth");

    assert_eq!(result.updated_users, 2);
    assert_eq!(result.current_method, "openid");
    assert_eq!(result.new_method, "basic");
    assert_eq!(
        mock.updated_users.lock().expect("lock poisoned").as_slice(),
        [1, 2]
    );
}

#[tokio::test]
async fn test_enforce_mfa_with_group_id_and_auth_filter() {
    let mut users_by_id = HashMap::new();
    users_by_id.insert(11, user_data_ad(11, "u11", 9));
    users_by_id.insert(12, user_data_ad(12, "u12", 9));
    users_by_id.insert(13, user_data_basic(13, "u13"));

    let mock = Arc::new(
        MockUsersApi::new(vec![empty_users()], HashMap::new(), users_by_id).with_group_user_pages(
            vec![group_users_page(
                vec![
                    group_user(11, "u11"),
                    group_user(12, "u12"),
                    group_user(13, "u13"),
                ],
                3,
            )],
        ),
    );
    let service = UsersService::new(mock.clone());

    let result = service
        .enforce_mfa(Some("ad".to_string()), None, Some(9), Some(777))
        .await
        .expect("enforce mfa");

    assert_eq!(result.success_count, 2);
    assert_eq!(result.failed_count, 0);
    assert_eq!(
        mock.updated_users.lock().expect("lock poisoned").as_slice(),
        [11, 12]
    );
}

#[tokio::test]
async fn test_enforce_mfa_counts_failures() {
    let mut users_by_id = HashMap::new();
    users_by_id.insert(1, user_data_basic(1, "u1"));
    users_by_id.insert(2, user_data_basic(2, "u2"));

    let mut failed = HashSet::new();
    failed.insert(2);

    let mock = Arc::new(
        MockUsersApi::new(
            vec![users_page(vec![user_item(1, "u1"), user_item(2, "u2")], 2)],
            HashMap::new(),
            users_by_id,
        )
        .with_failed_updates(failed),
    );
    let service = UsersService::new(mock);

    let result = service
        .enforce_mfa(None, Some("userName:cn:u".to_string()), None, None)
        .await
        .expect("enforce mfa");

    assert_eq!(result.success_count, 1);
    assert_eq!(result.failed_count, 1);
}

#[tokio::test]
async fn test_enforce_mfa_requires_selector() {
    let mock = Arc::new(MockUsersApi::new(
        vec![empty_users()],
        HashMap::new(),
        HashMap::new(),
    ));
    let service = UsersService::new(mock);

    let err = service
        .enforce_mfa(None, None, None, None)
        .await
        .expect_err("expected error");

    assert_eq!(
            err,
            DcCmdError::InvalidArgument(
                "Either auth method, group or filter must be provided. This would enforce MFA for all users and can be achieved via system settings.".to_string()
            )
        );
}
