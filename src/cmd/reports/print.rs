use chrono::SecondsFormat;
use dco3::eventlog::{AuditNodeList, LogEventList, LogOperationList};
use tabled::settings::{Panel, Style};

use crate::cmd::models::DcCmdError;

use super::{
    models::{EventOperationInfo, LogEventInfo, UserPermissionInfo},
    ReportsCommandHandler,
};

impl ReportsCommandHandler {
    pub fn print_events(&self, events: LogEventList, csv: bool) -> Result<(), DcCmdError> {
        if csv {
            self.print_events_csv(events)
        } else {
            self.print_events_table(events)
        }
    }

    pub fn print_permissions(&self, perms: AuditNodeList, csv: bool) -> Result<(), DcCmdError> {
        if csv {
            self.print_permissions_csv(perms)
        } else {
            self.print_permissions_table(perms)
        }
    }

    fn print_events_csv(&self, events: LogEventList) -> Result<(), DcCmdError> {
        const NOT_AVAILABLE: &str = "N/A";
        let header = "id,time,user_id,message,operation_id,operation_name,status,user_client,user_name,customer_id,auth_parent_source,auth_parent_target,object_id1,object_id2,object_type1,object_type2,object_name1,object_name2,attribute1,attribute2,attribute3";

        self.term
            .write_line(header)
            .map_err(|_| DcCmdError::IoError)?;

        for event in events.items {
            let id = event.id.to_string();
            let time = event.time.to_rfc3339_opts(SecondsFormat::Secs, true);
            let user_id = event.user_id.to_string();
            let message = event.message;
            let operation_id = event
                .operation_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let operation_name = event
                .operation_name
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let status = event
                .status
                .map(|s| i64::from(s).to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let user_client = event
                .user_client
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let user_name = event.user_name.unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let customer_id = event
                .customer_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let auth_parent_source = event
                .auth_parent_source
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let auth_parent_target = event
                .auth_parent_target
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_id1 = event
                .object_id1
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_id2 = event
                .object_id2
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_type1 = event
                .object_type1
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_type2 = event
                .object_type2
                .map(|id| id.to_string())
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_name1 = event
                .object_name1
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let object_name2 = event
                .object_name2
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let attribute1 = event
                .attribute1
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let attribute2 = event
                .attribute2
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());
            let attribute3 = event
                .attribute3
                .unwrap_or_else(|| NOT_AVAILABLE.to_string());

            let line = format!(
            "{id},{time},{user_id},{message},{operation_id},{operation_name},{status},{user_client},{user_name},{customer_id},{auth_parent_source},{auth_parent_target},{object_id1},{object_id2},{object_type1},{object_type2},{object_name1},{object_name2},{attribute1},{attribute2},{attribute3}",
        );

            self.term
                .write_line(&line)
                .map_err(|_| DcCmdError::IoError)?;
        }

        Ok(())
    }

    fn print_events_table(&self, events: LogEventList) -> Result<(), DcCmdError> {
        let event_count = events.items.len();
        let total_events = events.range.total;
        let events = events
            .items
            .iter()
            .map(|event| {
                LogEventInfo::new(
                    event.id,
                    event.time.to_rfc3339_opts(SecondsFormat::Secs, true),
                    event.user_id,
                    event.message.clone(),
                )
            })
            .collect::<Vec<_>>();

        let mut table = tabled::Table::new(events);
        table.with(Style::modern()).with(Panel::footer(format!(
            "{} events ({} total)",
            event_count, total_events
        )));

        self.term
            .write_line(&table.to_string())
            .map_err(|_| DcCmdError::IoError)?;
        Ok(())
    }

    fn print_permissions_table(&self, perms: AuditNodeList) -> Result<(), DcCmdError> {
        let permission_count = perms.len();

        let perms = perms
            .iter()
            .flat_map(|perm| {
                perm.audit_user_permission_list
                    .iter()
                    .map(move |user_perm| {
                        UserPermissionInfo::new(
                            user_perm.user_id,
                            user_perm.user_login.clone(),
                            format!("{} {}", user_perm.user_first_name, user_perm.user_last_name),
                            perm.node_id,
                            perm.node_name.clone(),
                            perm.node_parent_path.clone(),
                            user_perm.permissions.clone(),
                        )
                    })
            })
            .collect::<Vec<_>>();

        let mut table = tabled::Table::new(perms);
        table
            .with(Style::modern())
            .with(Panel::footer(format!("{} permissions", permission_count)));
        self.term
            .write_line(&table.to_string())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }

    fn print_permissions_csv(&self, perms: AuditNodeList) -> Result<(), DcCmdError> {
        let header = "nodeId,nodeName,nodeParentPath,userId,userLogin,userFirstName,userLastName,manage,read,create,change,delete,manageDownloadShare,manageUploadShare,readRecycleBin,restoreRecycleBin,deleteRecycleBin";

        self.term
            .write_line(header)
            .map_err(|_| DcCmdError::IoError)?;

        for perm in perms {
            let node_id = perm.node_id.to_string();
            let node_name = perm.node_name;
            let node_parent_path = perm.node_parent_path;
            let permission = perm.audit_user_permission_list.first();
            if permission.is_none() {
                continue;
            }
            let user_perms = permission.unwrap();
            let user_id = user_perms.user_id.to_string();
            let user_login = &user_perms.user_login;
            let user_first_name = &user_perms.user_first_name;
            let user_last_name = &user_perms.user_last_name;
            let manage = user_perms.permissions.manage.to_string();
            let read = user_perms.permissions.read.to_string();
            let create = user_perms.permissions.create.to_string();
            let change = user_perms.permissions.change.to_string();
            let delete = user_perms.permissions.delete.to_string();
            let manage_download_share = user_perms.permissions.manage_download_share.to_string();
            let manage_upload_share = user_perms.permissions.manage_upload_share.to_string();
            let read_recycle_bin = user_perms.permissions.read_recycle_bin.to_string();
            let restore_recycle_bin = user_perms.permissions.restore_recycle_bin.to_string();
            let delete_recycle_bin = user_perms.permissions.delete_recycle_bin.to_string();

            let line = format!(
                "{node_id},{node_name},{node_parent_path},{user_id},{user_login},{user_first_name},{user_last_name},{manage},{read},{create},{change},{delete},{manage_download_share},{manage_upload_share},{read_recycle_bin},{restore_recycle_bin},{delete_recycle_bin}",
            );

            self.term
                .write_line(&line)
                .map_err(|_| DcCmdError::IoError)?;
        }

        Ok(())
    }

    pub fn print_event_types(&self, operations: LogOperationList) -> Result<(), DcCmdError> {
        let event_types = operations
            .operation_list
            .iter()
            .map(|op| EventOperationInfo::from(op.clone()))
            .collect::<Vec<_>>();

        let mut table = tabled::Table::new(event_types);
        table.with(Style::modern());

        self.term
            .write_line(&table.to_string())
            .map_err(|_| DcCmdError::IoError)?;

        Ok(())
    }
}
