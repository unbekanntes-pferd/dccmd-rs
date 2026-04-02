#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupsRequest {
    Ls {
        target: String,
        filter: Option<String>,
        offset: Option<u64>,
        limit: Option<u32>,
        all: bool,
        csv: bool,
    },
    Create {
        target: String,
        name: String,
    },
    Rm {
        target: String,
        group_name: Option<String>,
        group_id: Option<u64>,
    },
    Users {
        cmd: GroupsUsersRequest,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupsUsersRequest {
    Ls {
        target: String,
        filter: Option<String>,
        offset: Option<u32>,
        limit: Option<u32>,
        all: bool,
        csv: bool,
    },
    Add {
        target: String,
        group_name: Option<String>,
        group_id: Option<u64>,
        user_name: Option<String>,
        user_id: Option<u64>,
    },
}
