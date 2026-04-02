#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportsRequest {
    Events {
        target: String,
        filter: Option<String>,
        offset: Option<u64>,
        limit: Option<u32>,
        all: bool,
        csv: bool,
        operation_type: Option<u64>,
        user_id: Option<u64>,
        status: Option<u8>,
        start_date: Option<String>,
        end_date: Option<String>,
    },
    OperationTypes {
        target: String,
    },
    Permissions {
        target: String,
        filter: Option<String>,
        offset: Option<u64>,
        limit: Option<u32>,
        all: bool,
        csv: bool,
    },
}
