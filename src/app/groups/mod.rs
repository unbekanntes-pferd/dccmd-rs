pub mod api;

use std::collections::HashSet;

use dco3::{
    groups::{Group, GroupUser, GroupsFilter},
    ListAllParams, RangedItems,
};
use tracing::error;

use crate::{
    app::{groups::api::GroupsApi, users::api::UsersApi},
    core::models::{build_params, DcCmdError, ListOptions},
};

pub struct GroupsService<C: GroupsApi> {
    api: C,
}

pub struct GroupUsersPage {
    pub group: Group,
    pub users: RangedItems<GroupUser>,
}

impl<C: GroupsApi> GroupsService<C> {
    pub fn new(api: C) -> Self {
        Self { api }
    }

    pub async fn create_group(&self, name: &str) -> Result<Group, DcCmdError> {
        self.api.create_group(name).await
    }

    pub async fn list_groups(&self, opts: &ListOptions) -> Result<RangedItems<Group>, DcCmdError> {
        let params = build_params(
            opts.filter(),
            opts.offset().unwrap_or(0),
            Some(opts.limit().unwrap_or(500)),
        )?;

        let mut groups = self.api.get_groups(Some(params)).await?;

        if opts.all() && groups.range.total > 500 {
            let mut page_offset = 500;
            while page_offset <= groups.range.total {
                let params = build_params(opts.filter(), page_offset, Some(500))?;
                let next_page = self.api.get_groups(Some(params)).await?;
                groups.items.extend(next_page.items);
                page_offset += 500;
            }
        }

        Ok(groups)
    }

    pub async fn delete_group(
        &self,
        group_name: Option<String>,
        group_id: Option<u64>,
    ) -> Result<u64, DcCmdError> {
        let group_id = self.get_group(group_name, group_id).await?.id;

        self.api.delete_group(group_id).await?;

        Ok(group_id)
    }

    pub async fn get_group(
        &self,
        group_name: Option<String>,
        group_id: Option<u64>,
    ) -> Result<Group, DcCmdError> {
        match (group_name, group_id) {
            (_, Some(id)) => self.api.get_group(id).await,
            (Some(name), None) => self.find_group_by_name(&name).await,
            _ => Err(DcCmdError::InvalidArgument(
                "Either group name or id must be provided".to_string(),
            )),
        }
    }

    pub async fn list_group_users(
        &self,
        group_name: Option<&str>,
        filter: &Option<String>,
        offset: Option<u32>,
        limit: Option<u32>,
        all: bool,
    ) -> Result<Vec<GroupUsersPage>, DcCmdError> {
        self.list_group_users_by_selector(group_name, None, filter, offset, limit, all)
            .await
    }

    pub async fn list_group_users_by_selector(
        &self,
        group_name: Option<&str>,
        group_id: Option<u64>,
        filter: &Option<String>,
        offset: Option<u32>,
        limit: Option<u32>,
        all: bool,
    ) -> Result<Vec<GroupUsersPage>, DcCmdError> {
        let groups = if let Some(group_id) = group_id {
            vec![self.api.get_group(group_id).await?]
        } else if let Some(group_name) = group_name.filter(|name| !name.is_empty()) {
            vec![self.find_group_by_name(group_name).await?]
        } else {
            self.list_all_groups().await?
        };

        let mut pages = Vec::with_capacity(groups.len());
        for group in groups {
            let mut users = self
                .api
                .get_group_users(
                    group.id,
                    Some(build_params(
                        filter,
                        u64::from(offset.unwrap_or(0)),
                        Some(limit.unwrap_or(500)),
                    )?),
                )
                .await?;

            if all {
                for page_offset in (500..=users.range.total).step_by(500) {
                    let params = build_params(filter, page_offset, Some(limit.unwrap_or(500)))?;
                    let next_page = self.api.get_group_users(group.id, Some(params)).await?;
                    users.items.extend(next_page.items);
                }
            }

            pages.push(GroupUsersPage { group, users });
        }

        Ok(pages)
    }

    async fn find_group_by_name(&self, name: &str) -> Result<Group, DcCmdError> {
        let params = ListAllParams::builder()
            .with_filter(GroupsFilter::name_contains(name))
            .build();
        let groups = self.api.get_groups(Some(params)).await?;

        let Some(group) = groups.items.into_iter().find(|g| g.name == name) else {
            error!("No group found with name: {name}");
            let msg = format!("No group found with name: {name}");
            return Err(DcCmdError::InvalidArgument(msg));
        };

        Ok(group)
    }

    async fn list_all_groups(&self) -> Result<Vec<Group>, DcCmdError> {
        let mut groups = self.api.get_groups(None).await?;

        for page_offset in (500..=groups.range.total).step_by(500) {
            let params = ListAllParams::builder()
                .with_offset(page_offset)
                .with_limit(500)
                .build();
            let next_page = self.api.get_groups(Some(params)).await?;
            groups.items.extend(next_page.items);
        }

        Ok(groups.items)
    }
}

impl<C: GroupsApi + UsersApi> GroupsService<C> {
    pub async fn add_group_users(
        &self,
        group_name: Option<String>,
        group_id: Option<u64>,
        user_names: Vec<String>,
        user_ids: Vec<u64>,
    ) -> Result<(Group, Vec<u64>), DcCmdError> {
        let group = self.get_group(group_name, group_id).await?;
        let user_ids = self.resolve_user_ids(user_names, user_ids).await?;
        let updated_group = self.api.add_group_users(group.id, user_ids.clone()).await?;

        Ok((updated_group, user_ids))
    }

    pub async fn add_group_user(
        &self,
        group_name: Option<String>,
        group_id: Option<u64>,
        user_name: Option<String>,
        user_id: Option<u64>,
    ) -> Result<(u64, u64), DcCmdError> {
        let (group, user_ids) = self
            .add_group_users(
                group_name,
                group_id,
                user_name.into_iter().collect(),
                user_id.into_iter().collect(),
            )
            .await?;
        let user_id = *user_ids
            .first()
            .expect("single-user add must resolve one user id");

        Ok((group.id, user_id))
    }

    async fn resolve_user_ids(
        &self,
        user_names: Vec<String>,
        user_ids: Vec<u64>,
    ) -> Result<Vec<u64>, DcCmdError> {
        let mut resolved = Vec::new();
        let mut seen = HashSet::new();

        for user_id in user_ids {
            if seen.insert(user_id) {
                resolved.push(user_id);
            }
        }

        for user_name in user_names {
            let user_name = user_name.trim();
            if user_name.is_empty() {
                continue;
            }

            let user_id = self.api.find_user_id_by_username(user_name).await?;
            if seen.insert(user_id) {
                resolved.push(user_id);
            }
        }

        if resolved.is_empty() {
            return Err(DcCmdError::InvalidArgument(
                "Either user name or id must be provided".to_string(),
            ));
        }

        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use async_trait::async_trait;
    use dco3::{
        groups::{Group, GroupUser},
        nodes::models::{UserInfo, UserType},
        ListAllParams, RangedItems,
    };

    use crate::{
        app::{groups::api::GroupsApi, users::api::UsersApi},
        core::models::ListOptions,
    };

    use super::GroupsService;

    struct MockGroupsApi {
        pages: Mutex<Vec<RangedItems<Group>>>,
        group_user_pages: Mutex<HashMap<u64, Vec<RangedItems<GroupUser>>>>,
        users_by_name: HashMap<String, u64>,
        created: Mutex<Vec<String>>,
        deleted: Mutex<Vec<u64>>,
        added_group_users: Mutex<Vec<(u64, Vec<u64>)>>,
        get_calls: Mutex<u32>,
    }

    impl MockGroupsApi {
        fn new(
            pages: Vec<RangedItems<Group>>,
            group_user_pages: HashMap<u64, Vec<RangedItems<GroupUser>>>,
            users_by_name: HashMap<String, u64>,
        ) -> Self {
            Self {
                pages: Mutex::new(pages),
                group_user_pages: Mutex::new(group_user_pages),
                users_by_name,
                created: Mutex::new(Vec::new()),
                deleted: Mutex::new(Vec::new()),
                added_group_users: Mutex::new(Vec::new()),
                get_calls: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl GroupsApi for MockGroupsApi {
        async fn get_groups(
            &self,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<Group>, crate::core::models::DcCmdError> {
            let mut calls = self.get_calls.lock().expect("lock poisoned");
            *calls += 1;
            drop(calls);

            let mut pages = self.pages.lock().expect("lock poisoned");
            if pages.len() > 1 {
                Ok(pages.remove(0))
            } else {
                Ok(pages.first().cloned().unwrap_or_else(empty_groups))
            }
        }

        async fn get_group(&self, group_id: u64) -> Result<Group, crate::core::models::DcCmdError> {
            let pages = self.pages.lock().expect("lock poisoned");
            pages
                .iter()
                .flat_map(|page| page.items.iter())
                .find(|group| group.id == group_id)
                .cloned()
                .ok_or_else(|| {
                    crate::core::models::DcCmdError::InvalidArgument(format!(
                        "No group found with id: {group_id}"
                    ))
                })
        }

        async fn create_group(&self, name: &str) -> Result<Group, crate::core::models::DcCmdError> {
            self.created
                .lock()
                .expect("lock poisoned")
                .push(name.to_string());
            Ok(group(99, name))
        }

        async fn delete_group(&self, group_id: u64) -> Result<(), crate::core::models::DcCmdError> {
            self.deleted.lock().expect("lock poisoned").push(group_id);
            Ok(())
        }

        async fn get_group_users(
            &self,
            group_id: u64,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<GroupUser>, crate::core::models::DcCmdError> {
            let mut map = self.group_user_pages.lock().expect("lock poisoned");
            let pages = map.get_mut(&group_id);
            if let Some(pages) = pages {
                if pages.len() > 1 {
                    Ok(pages.remove(0))
                } else {
                    Ok(pages.first().cloned().unwrap_or_else(empty_group_users))
                }
            } else {
                Ok(empty_group_users())
            }
        }

        async fn add_group_users(
            &self,
            group_id: u64,
            user_ids: Vec<u64>,
        ) -> Result<Group, crate::core::models::DcCmdError> {
            self.added_group_users
                .lock()
                .expect("lock poisoned")
                .push((group_id, user_ids));
            self.get_group(group_id).await
        }
    }

    #[async_trait]
    impl UsersApi for MockGroupsApi {
        async fn find_user_id_by_username(
            &self,
            user_name: &str,
        ) -> Result<u64, crate::core::models::DcCmdError> {
            self.users_by_name.get(user_name).copied().ok_or_else(|| {
                crate::core::models::DcCmdError::InvalidArgument(format!(
                    "No user found with username: {user_name}"
                ))
            })
        }
    }

    fn group(id: u64, name: &str) -> Group {
        Group {
            id,
            name: name.to_string(),
            cnt_users: Some(0),
            created_at: chrono::Utc::now(),
            updated_at: None,
            created_by: UserInfo {
                id: 1,
                user_type: UserType::Internal,
                user_name: Some("user".to_string()),
                first_name: None,
                last_name: None,
                email: None,
                avatar_uuid: "avatar".to_string(),
            },
            updated_by: None,
            expire_at: None,
            group_roles: None,
        }
    }

    fn groups_page(items: Vec<Group>, total: u64) -> RangedItems<Group> {
        RangedItems {
            range: dco3::models::Range {
                offset: 0,
                limit: 500,
                total,
            },
            items,
        }
    }

    fn empty_groups() -> RangedItems<Group> {
        groups_page(Vec::new(), 0)
    }

    fn group_user(id: i64, user_name: &str) -> GroupUser {
        GroupUser {
            user_info: UserInfo {
                id,
                user_type: UserType::Internal,
                user_name: Some(user_name.to_string()),
                avatar_uuid: "avatar".to_string(),
                first_name: None,
                last_name: None,
                email: None,
            },
            is_member: true,
        }
    }

    fn group_user_page(items: Vec<GroupUser>, total: u64) -> RangedItems<GroupUser> {
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
        group_user_page(Vec::new(), 0)
    }

    #[tokio::test]
    async fn test_create_group() {
        let api = MockGroupsApi::new(vec![empty_groups()], HashMap::new(), HashMap::new());
        let service = GroupsService::new(api);

        let group = service.create_group("team-a").await.unwrap();

        assert_eq!(group.name, "team-a");
        assert_eq!(
            service
                .api
                .created
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            ["team-a"]
        );
    }

    #[tokio::test]
    async fn test_list_groups_all_fetches_pages() {
        let api = MockGroupsApi::new(
            vec![
                groups_page(vec![group(1, "a")], 700),
                groups_page(vec![group(2, "b")], 700),
            ],
            HashMap::new(),
            HashMap::new(),
        );
        let service = GroupsService::new(api);

        let result = service
            .list_groups(&ListOptions::new(None, None, None, true, false))
            .await
            .unwrap();

        assert_eq!(result.items.len(), 2);
        assert_eq!(*service.api.get_calls.lock().expect("lock poisoned"), 2);
    }

    #[tokio::test]
    async fn test_delete_group_by_id() {
        let api = MockGroupsApi::new(vec![empty_groups()], HashMap::new(), HashMap::new());
        let service = GroupsService::new(api);

        let deleted_id = service.delete_group(None, Some(55)).await.unwrap();

        assert_eq!(deleted_id, 55);
        assert_eq!(
            service
                .api
                .deleted
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            [55]
        );
    }

    #[tokio::test]
    async fn test_delete_group_by_name() {
        let api = MockGroupsApi::new(
            vec![groups_page(vec![group(9, "team-a")], 1)],
            HashMap::new(),
            HashMap::new(),
        );
        let service = GroupsService::new(api);

        let deleted_id = service
            .delete_group(Some("team-a".to_string()), None)
            .await
            .unwrap();

        assert_eq!(deleted_id, 9);
        assert_eq!(
            service
                .api
                .deleted
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            [9]
        );
    }

    #[tokio::test]
    async fn test_delete_group_requires_name_or_id() {
        let api = MockGroupsApi::new(vec![empty_groups()], HashMap::new(), HashMap::new());
        let service = GroupsService::new(api);

        let result = service.delete_group(None, None).await;

        assert!(matches!(
            result,
            Err(crate::core::models::DcCmdError::InvalidArgument(msg))
            if msg == "Either group name or id must be provided"
        ));
    }

    #[tokio::test]
    async fn test_delete_group_by_name_not_found() {
        let api = MockGroupsApi::new(
            vec![groups_page(vec![group(9, "other")], 1)],
            HashMap::new(),
            HashMap::new(),
        );
        let service = GroupsService::new(api);

        let result = service.delete_group(Some("team-a".to_string()), None).await;

        assert!(matches!(
            result,
            Err(crate::core::models::DcCmdError::InvalidArgument(msg))
            if msg == "No group found with name: team-a"
        ));
    }

    #[tokio::test]
    async fn test_list_group_users_for_single_group_name() {
        let api = MockGroupsApi::new(
            vec![groups_page(vec![group(9, "team-a")], 1)],
            HashMap::from([(
                9_u64,
                vec![group_user_page(vec![group_user(1, "alice")], 1)],
            )]),
            HashMap::new(),
        );
        let service = GroupsService::new(api);

        let pages = service
            .list_group_users(Some("team-a"), &None, None, None, false)
            .await
            .unwrap();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].group.id, 9);
        assert_eq!(pages[0].users.items.len(), 1);
        assert_eq!(
            pages[0].users.items[0].user_info.user_name.as_deref(),
            Some("alice")
        );
    }

    #[tokio::test]
    async fn test_add_group_user_resolves_names() {
        let api = MockGroupsApi::new(
            vec![groups_page(vec![group(9, "team-a")], 1)],
            HashMap::new(),
            HashMap::from([(String::from("alice"), 77_u64)]),
        );
        let service = GroupsService::new(api);

        let (group_id, user_id) = service
            .add_group_user(
                Some("team-a".to_string()),
                None,
                Some("alice".to_string()),
                None,
            )
            .await
            .unwrap();

        assert_eq!(group_id, 9);
        assert_eq!(user_id, 77);
        assert_eq!(
            service
                .api
                .added_group_users
                .lock()
                .expect("lock poisoned")
                .as_slice(),
            &[(9, vec![77])]
        );
    }

    #[tokio::test]
    async fn test_add_group_user_requires_user_name_or_id() {
        let api = MockGroupsApi::new(vec![empty_groups()], HashMap::new(), HashMap::new());
        let service = GroupsService::new(api);

        let result = service
            .add_group_user(Some("team-a".to_string()), Some(9), None, None)
            .await;

        assert!(matches!(
            result,
            Err(crate::core::models::DcCmdError::InvalidArgument(msg))
            if msg == "Either user name or id must be provided"
        ));
    }
}
