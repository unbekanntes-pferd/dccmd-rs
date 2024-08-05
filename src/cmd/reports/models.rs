use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use dco3::{
    eventlog::{EventStatus, EventlogParams, LogOperation},
    nodes::NodePermissions,
};
use tabled::Tabled;

use crate::cmd::models::{DcCmdError, ListOptions};

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
                    .map_err(|e| DcCmdError::InvalidArgument(format!("Invalid start date: {}", e)))
                    .map(|date| {
                        let time = NaiveTime::from_hms_opt(0, 0, 0).unwrap(); // Midnight
                        let naive_datetime = date.and_time(time);
                        DateTime::<Utc>::from_naive_utc_and_offset(naive_datetime, Utc)
                    })
            })
            .transpose()?;

        let end_date = end_date
            .map(|s| {
                NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                    .map_err(|e| DcCmdError::InvalidArgument(format!("Invalid end date: {}", e)))
                    .map(|date| {
                        let time = NaiveTime::from_hms_opt(23, 59, 59).unwrap(); // End of day
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

#[derive(Tabled)]
pub struct LogEventInfo {
    id: i64,
    time: String,
    user_id: i64,
    message: String,
}

impl LogEventInfo {
    pub fn new(id: i64, time: String, user_id: i64, message: String) -> Self {
        Self {
            id,
            time,
            user_id,
            message,
        }
    }
}

#[derive(Tabled)]
pub struct UserPermissionInfo {
    user_id: i64,
    user_login: String,
    user_full_name: String,
    node_id: i64,
    node_name: String,
    node_parent_path: String,
    permissions: String,
}

impl UserPermissionInfo {
    pub fn new(
        user_id: i64,
        user_login: String,
        user_full_name: String,
        node_id: i64,
        node_name: String,
        node_parent_path: String,
        permissions: NodePermissions,
    ) -> Self {
        Self {
            user_id,
            user_login,
            user_full_name,
            node_id,
            node_name,
            node_parent_path,
            permissions: permissions.to_string(),
        }
    }
}

#[derive(Tabled)]
pub struct EventOperationInfo {
    id: i64,
    name: String,
    deprecated: bool,
}

impl From<LogOperation> for EventOperationInfo {
    fn from(op: LogOperation) -> Self {
        Self {
            id: op.id,
            name: op.name,
            deprecated: op.is_deprecated,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Datelike;

    use super::EventOptions;

    #[test]
    fn test_create_event_options_with_start_and_end_date() {
        let start_date = Some("2021-01-01".to_string());
        let end_date = Some("2021-01-31".to_string());

        let event_options =
            EventOptions::new(Default::default(), start_date, end_date, None, None, None).unwrap();

        assert_eq!(event_options.start_date.is_some(), true);
        assert_eq!(event_options.end_date.is_some(), true);

        let start_date = event_options.start_date.unwrap();
        let end_date = event_options.end_date.unwrap();

        assert_eq!(start_date.year(), 2021);
        assert_eq!(start_date.month(), 1);
        assert_eq!(start_date.day(), 1);

        assert_eq!(end_date.year(), 2021);
        assert_eq!(end_date.month(), 1);
        assert_eq!(end_date.day(), 31);
    }

    #[test]
    fn test_create_event_options_with_status() {
        let status = Some(0);

        let event_options =
            EventOptions::new(Default::default(), None, None, None, None, status).unwrap();

        assert_eq!(event_options.status.is_some(), true);
    }

    #[test]
    fn test_create_params_with_new_offset() {
        let event_options =
            EventOptions::new(Default::default(), None, None, None, None, None).unwrap();

        let params = event_options.new_params_with_offset(500);

        assert_eq!(params.offset, Some(500));
    }
}
