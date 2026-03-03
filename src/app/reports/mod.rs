pub mod api;

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use dco3::{
    eventlog::{
        AuditNodeList, AuditNodesFilter, EventStatus, EventlogParams, LogEventList,
        LogOperationList,
    },
    FilterQuery, ListAllParams,
};
use tracing::{error, warn};

use crate::{
    app::reports::api::ReportsApi,
    core::models::{build_params, DcCmdError, ListOptions},
};

#[derive(Clone)]
pub struct EventOptions {
    pub list_options: ListOptions,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub user_id: Option<u64>,
    pub operation_type: Option<u64>,
    pub status: Option<EventStatus>,
}

impl EventOptions {
    pub fn new(
        list_options: ListOptions,
        start_date: Option<String>,
        end_date: Option<String>,
        user_id: Option<u64>,
        operation_type: Option<u64>,
        status: Option<u8>,
    ) -> Result<Self, DcCmdError> {
        let start_date = start_date
            .map(|s| {
                NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                    .map_err(|e| DcCmdError::InvalidArgument(format!("Invalid start date: {e}")))
                    .map(|date| {
                        let time = NaiveTime::from_hms_opt(0, 0, 0).expect("valid midnight time");
                        let naive_datetime = date.and_time(time);
                        DateTime::<Utc>::from_naive_utc_and_offset(naive_datetime, Utc)
                    })
            })
            .transpose()?;

        let end_date = end_date
            .map(|s| {
                NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                    .map_err(|e| DcCmdError::InvalidArgument(format!("Invalid end date: {e}")))
                    .map(|date| {
                        let time =
                            NaiveTime::from_hms_opt(23, 59, 59).expect("valid end-of-day time");
                        let naive_datetime = date.and_time(time);
                        DateTime::<Utc>::from_naive_utc_and_offset(naive_datetime, Utc)
                    })
            })
            .transpose()?;

        let status = status
            .map(|s| {
                EventStatus::try_from(s as i64)
                    .map_err(|_| DcCmdError::InvalidArgument("Invalid status".to_string()))
            })
            .transpose()?;

        Ok(Self {
            list_options,
            start_date,
            end_date,
            user_id,
            operation_type,
            status,
        })
    }

    pub fn new_params_with_offset(&self, offset: u64) -> EventlogParams {
        let mut params: EventlogParams = self.clone().into();
        params.offset = Some(offset);
        params
    }
}

impl From<EventOptions> for EventlogParams {
    fn from(value: EventOptions) -> Self {
        let params_builder = EventlogParams::builder();

        let params_builder = if let Some(start_date) = value.start_date {
            params_builder.with_date_start(start_date)
        } else {
            params_builder
        };

        let params_builder = if let Some(end_date) = value.end_date {
            params_builder.with_date_end(end_date)
        } else {
            params_builder
        };

        let params_builder = if let Some(user_id) = value.user_id {
            params_builder.with_user_id(user_id as i64)
        } else {
            params_builder
        };

        let params_builder = if let Some(operation_type) = value.operation_type {
            params_builder.with_operation_type(operation_type as i64)
        } else {
            params_builder
        };

        let params_builder = if let Some(status) = value.status {
            params_builder.with_status(status)
        } else {
            params_builder
        };

        let params_builder = if let Some(offset) = value.list_options.offset() {
            params_builder.with_offset(offset)
        } else {
            params_builder
        };

        let params_builder = if let Some(limit) = value.list_options.limit() {
            params_builder.with_limit(limit.into())
        } else {
            params_builder
        };

        params_builder.build()
    }
}

pub struct ReportsService<C: ReportsApi> {
    api: C,
}

impl<C: ReportsApi> ReportsService<C> {
    pub fn new(api: C) -> Self {
        Self { api }
    }

    pub async fn check_dracoon_api_version(&self) -> Result<(), DcCmdError> {
        let software_version = self.api.get_software_version().await?;
        let rest_api_version = software_version.rest_api_version;

        if let Some(major_version) = rest_api_version.split('.').next() {
            if let Ok(num) = major_version.parse::<u8>() {
                if num > 4 {
                    error!(
                        "Permissions report is only available for API version 4.x (DRACOON Server) - used version: {}",
                        rest_api_version
                    );
                    return Err(DcCmdError::InvalidArgument(
                        "Permissions report is only available for API version 4.x (DRACOON Server)"
                            .to_string(),
                    ));
                }
            }
        } else {
            warn!("Failed to parse API version");
        }

        Ok(())
    }

    pub async fn get_events(&self, opts: EventOptions) -> Result<LogEventList, DcCmdError> {
        let params = opts.clone().into();
        let mut event_list = self.api.get_events(params).await?;

        if opts.list_options.all() {
            let reqs = (500..=event_list.range.total)
                .step_by(500)
                .map(|offset| {
                    let params = opts.new_params_with_offset(offset);
                    self.api.get_events(params)
                })
                .collect::<Vec<_>>();

            for req in reqs {
                let next_events = req.await?;
                event_list.items.extend(next_events.items);
            }
        }

        Ok(event_list)
    }

    pub async fn get_event_operations(&self) -> Result<LogOperationList, DcCmdError> {
        self.api.get_event_operations().await
    }

    #[allow(deprecated)]
    pub async fn get_permissions(&self, opts: ListOptions) -> Result<AuditNodeList, DcCmdError> {
        let offset = opts.offset().unwrap_or(0);

        if let Some(filter) = opts.filter() {
            let params = build_params(&Some(filter.to_string()), offset, None)?;
            return self.api.get_node_permissions(params).await;
        }

        let user_ids = self.get_all_user_ids().await?;

        let mut perms = Vec::new();

        for user in user_ids {
            let user_filter = AuditNodesFilter::user_id_equals(user).to_filter_string();
            let params = build_params(&Some(user_filter), offset, None)?;
            let next_perms = self.api.get_node_permissions(params).await?;
            perms.extend(next_perms);
        }

        Ok(perms)
    }

    async fn get_all_user_ids(&self) -> Result<Vec<u64>, DcCmdError> {
        let mut users = self.api.get_users(None).await?;

        let user_reqs = (500..=users.range.total)
            .step_by(500)
            .map(|offset| {
                let params = ListAllParams::builder().with_offset(offset).build();
                self.api.get_users(Some(params))
            })
            .collect::<Vec<_>>();

        for user_req in user_reqs {
            let user = user_req.await?;
            users.items.extend(user.items);
        }

        let user_ids = users.items.iter().map(|u| u.id).collect::<Vec<_>>();

        Ok(user_ids)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use chrono::{Datelike, Utc};
    use dco3::{
        eventlog::{
            AuditNodeList, AuditNodeResponse, AuditUserPermission, EventStatus, LogEvent,
            LogEventList, LogOperation, LogOperationList,
        },
        models::Range,
        nodes::NodePermissions,
        public::SoftwareVersionData,
        users::UserItem,
        ListAllParams, RangedItems,
    };

    use crate::{app::reports::api::ReportsApi, core::models::ListOptions};

    use super::{EventOptions, ReportsService};

    struct MockReportsApi {
        software_version: String,
        events: Mutex<VecDeque<LogEventList>>,
        operations: Mutex<Vec<LogOperation>>,
        permissions: Mutex<VecDeque<AuditNodeList>>,
        users: Mutex<VecDeque<RangedItems<UserItem>>>,
        node_permission_filters: Mutex<Vec<String>>,
        user_calls: Mutex<u32>,
    }

    impl MockReportsApi {
        fn new(
            software_version: &str,
            events: Vec<LogEventList>,
            operations: Vec<LogOperation>,
            permissions: Vec<AuditNodeList>,
            users: Vec<RangedItems<UserItem>>,
        ) -> Self {
            Self {
                software_version: software_version.to_string(),
                events: Mutex::new(VecDeque::from(events)),
                operations: Mutex::new(operations),
                permissions: Mutex::new(VecDeque::from(permissions)),
                users: Mutex::new(VecDeque::from(users)),
                node_permission_filters: Mutex::new(Vec::new()),
                user_calls: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl ReportsApi for Arc<MockReportsApi> {
        async fn get_software_version(
            &self,
        ) -> Result<SoftwareVersionData, crate::core::models::DcCmdError> {
            Ok(SoftwareVersionData {
                rest_api_version: self.software_version.clone(),
                sds_server_version: "26.0.0".to_string(),
                build_date: Utc::now(),
                is_dracoon_cloud: Some(false),
            })
        }

        async fn get_events(
            &self,
            _params: dco3::eventlog::EventlogParams,
        ) -> Result<LogEventList, crate::core::models::DcCmdError> {
            let mut events = self.events.lock().expect("lock poisoned");
            if events.len() > 1 {
                Ok(events.pop_front().expect("at least one page"))
            } else {
                Ok(events.front().cloned().unwrap_or_else(empty_events))
            }
        }

        async fn get_event_operations(
            &self,
        ) -> Result<LogOperationList, crate::core::models::DcCmdError> {
            Ok(LogOperationList {
                operation_list: self.operations.lock().expect("lock poisoned").clone(),
            })
        }

        async fn get_node_permissions(
            &self,
            params: ListAllParams,
        ) -> Result<AuditNodeList, crate::core::models::DcCmdError> {
            self.node_permission_filters
                .lock()
                .expect("lock poisoned")
                .push(params.filter_to_string());

            let mut permissions = self.permissions.lock().expect("lock poisoned");
            if permissions.len() > 1 {
                Ok(permissions.pop_front().expect("at least one response"))
            } else {
                Ok(permissions.front().cloned().unwrap_or_default())
            }
        }

        async fn get_users(
            &self,
            _params: Option<ListAllParams>,
        ) -> Result<RangedItems<UserItem>, crate::core::models::DcCmdError> {
            let mut user_calls = self.user_calls.lock().expect("lock poisoned");
            *user_calls += 1;
            drop(user_calls);

            let mut users = self.users.lock().expect("lock poisoned");
            if users.len() > 1 {
                Ok(users.pop_front().expect("at least one page"))
            } else {
                Ok(users.front().cloned().unwrap_or_else(empty_users))
            }
        }
    }

    fn empty_events() -> LogEventList {
        events_page(Vec::new(), 0)
    }

    fn events_page(items: Vec<LogEvent>, total: u64) -> LogEventList {
        RangedItems {
            range: Range {
                offset: 0,
                limit: 500,
                total,
            },
            items,
        }
    }

    fn event(id: i64) -> LogEvent {
        LogEvent {
            id,
            time: Utc::now(),
            user_id: 1,
            message: format!("event-{id}"),
            operation_id: Some(1),
            operation_name: Some("op".to_string()),
            status: Some(EventStatus::Success),
            user_client: Some("cli".to_string()),
            customer_id: None,
            user_name: Some("user".to_string()),
            user_ip: None,
            auth_parent_source: None,
            auth_parent_target: None,
            object_id1: None,
            object_id2: None,
            object_type1: None,
            object_type2: None,
            object_name1: None,
            object_name2: None,
            attribute1: None,
            attribute2: None,
            attribute3: None,
        }
    }

    fn empty_users() -> RangedItems<UserItem> {
        users_page(Vec::new(), 0)
    }

    fn users_page(items: Vec<UserItem>, total: u64) -> RangedItems<UserItem> {
        RangedItems {
            range: Range {
                offset: 0,
                limit: 500,
                total,
            },
            items,
        }
    }

    fn user(id: u64, user_name: &str) -> UserItem {
        UserItem {
            id,
            user_name: user_name.to_string(),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            is_locked: false,
            avatar_uuid: "avatar".to_string(),
            email: None,
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

    fn permission(node_id: i64, user_id: i64) -> AuditNodeResponse {
        AuditNodeResponse {
            node_id,
            node_name: format!("node-{node_id}"),
            node_parent_path: "/".to_string(),
            node_cnt_children: 0,
            audit_user_permission_list: vec![AuditUserPermission {
                user_id,
                user_login: format!("user-{user_id}"),
                user_first_name: "First".to_string(),
                user_last_name: "Last".to_string(),
                permissions: NodePermissions::new_with_manage_permissions(),
            }],
            node_parent_id: None,
            node_size: None,
            node_recycle_bin_retention_period: None,
            node_quota: None,
            node_is_encrypted: None,
            node_has_activities_log: None,
            node_created_at: None,
            node_updated_at: None,
            node_created_by: None,
            node_updated_by: None,
        }
    }

    #[tokio::test]
    async fn test_check_dracoon_api_version_rejects_v5() {
        let mock = Arc::new(MockReportsApi::new(
            "5.4.0",
            vec![empty_events()],
            vec![],
            vec![],
            vec![empty_users()],
        ));
        let service = ReportsService::new(mock);

        let err = service
            .check_dracoon_api_version()
            .await
            .expect_err("expected error");

        match err {
            crate::core::models::DcCmdError::InvalidArgument(msg) => {
                assert!(msg.contains("Permissions report"));
            }
            _ => panic!("expected invalid argument error"),
        }
    }

    #[tokio::test]
    async fn test_get_events_collects_all_pages() {
        let mock = Arc::new(MockReportsApi::new(
            "4.22.0",
            vec![
                events_page(vec![event(1)], 501),
                events_page(vec![event(2)], 501),
            ],
            vec![],
            vec![],
            vec![empty_users()],
        ));
        let service = ReportsService::new(mock);
        let opts = EventOptions::new(
            ListOptions::new(None, None, None, true, false),
            None,
            None,
            None,
            None,
            None,
        )
        .expect("valid options");

        let events = service.get_events(opts).await.expect("events");

        assert_eq!(events.items.len(), 2);
        assert_eq!(events.items[0].id, 1);
        assert_eq!(events.items[1].id, 2);
    }

    #[tokio::test]
    async fn test_get_permissions_uses_filter_without_listing_users() {
        let mock = Arc::new(MockReportsApi::new(
            "4.22.0",
            vec![empty_events()],
            vec![],
            vec![vec![permission(1, 1)]],
            vec![empty_users()],
        ));
        let service = ReportsService::new(mock.clone());
        let opts = ListOptions::new(Some("userId:eq:1".to_string()), None, None, false, false);

        let perms = service.get_permissions(opts).await.expect("permissions");

        assert_eq!(perms.len(), 1);
        let filters = mock.node_permission_filters.lock().expect("lock poisoned");
        assert_eq!(filters.as_slice(), ["userId:eq:1"]);
        assert_eq!(*mock.user_calls.lock().expect("lock poisoned"), 0);
    }

    #[tokio::test]
    async fn test_get_permissions_collects_by_each_user_without_filter() {
        let mock = Arc::new(MockReportsApi::new(
            "4.22.0",
            vec![empty_events()],
            vec![],
            vec![
                vec![permission(10, 1)],
                vec![permission(11, 2)],
                vec![permission(12, 3)],
            ],
            vec![
                users_page(vec![user(1, "u1"), user(2, "u2")], 501),
                users_page(vec![user(3, "u3")], 501),
            ],
        ));
        let service = ReportsService::new(mock.clone());
        let opts = ListOptions::new(None, None, None, false, false);

        let perms = service.get_permissions(opts).await.expect("permissions");

        assert_eq!(perms.len(), 3);
        let filters = mock.node_permission_filters.lock().expect("lock poisoned");
        assert_eq!(
            filters.as_slice(),
            ["userId:eq:1", "userId:eq:2", "userId:eq:3"]
        );
        assert_eq!(*mock.user_calls.lock().expect("lock poisoned"), 2);
    }

    #[tokio::test]
    async fn test_get_event_operations_delegates_to_api() {
        let mock = Arc::new(MockReportsApi::new(
            "4.22.0",
            vec![empty_events()],
            vec![LogOperation {
                id: 42,
                name: "ROOM_CREATE".to_string(),
                is_deprecated: false,
            }],
            vec![],
            vec![empty_users()],
        ));
        let service = ReportsService::new(mock);

        let operations = service.get_event_operations().await.expect("operations");

        assert_eq!(operations.operation_list.len(), 1);
        assert_eq!(operations.operation_list[0].id, 42);
    }

    #[test]
    fn test_event_options_parses_start_and_end_dates() {
        let opts = EventOptions::new(
            Default::default(),
            Some("2021-01-01".to_string()),
            Some("2021-01-31".to_string()),
            None,
            None,
            None,
        )
        .expect("valid dates");

        let start = opts.start_date.expect("start date");
        let end = opts.end_date.expect("end date");
        assert_eq!(start.year(), 2021);
        assert_eq!(start.month(), 1);
        assert_eq!(start.day(), 1);
        assert_eq!(end.year(), 2021);
        assert_eq!(end.month(), 1);
        assert_eq!(end.day(), 31);
    }

    #[test]
    fn test_event_options_parses_status() {
        let opts = EventOptions::new(Default::default(), None, None, None, None, Some(0))
            .expect("valid status");
        assert_eq!(opts.status, Some(EventStatus::Success));
    }

    #[test]
    fn test_event_options_new_params_with_offset() {
        let opts = EventOptions::new(Default::default(), None, None, None, None, None)
            .expect("valid opts");
        let params = opts.new_params_with_offset(500);
        assert_eq!(params.offset, Some(500));
    }
}
