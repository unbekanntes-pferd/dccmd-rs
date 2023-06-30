use crate::api::{models::{FilterOperator, FilterQuery}, auth::errors::DracoonClientError};

use super::NodeType;

#[derive(Debug)]
pub enum NodesFilter {
    Name(FilterOperator, String),
    Type(FilterOperator, NodeType),
    Encrypted(FilterOperator, bool),
    BranchVersion(FilterOperator, u64),
    TimestampCreation(FilterOperator, String),
    TimestampModification(FilterOperator, String),
    ReferenceId(FilterOperator, u64),
    // missing: perm, childPerm 
    // TODO: add permission model enum in api/models.rs
}

impl FilterQuery for NodesFilter {
    fn filter_to_string(&self) -> String {
        match self {
            NodesFilter::Name(op, value) => {
                let op = String::from(op);
                format!("name:{}:{}", op, value)
            },
            NodesFilter::Type(op, value) => {
                let op = String::from(op);
                let node_type = String::from(value);
                format!("type:{}:{}", op, node_type)
            },
            NodesFilter::Encrypted(op, value) => {
                let op = String::from(op);
                format!("encrypted:{}:{}", op, value)
            },
            NodesFilter::BranchVersion(op, value) => {
                let op = String::from(op);
                format!("branchVersion:{}:{}", op, value)
            },
            NodesFilter::TimestampCreation(op, value) => {
                let op = String::from(op);
                format!("timestampCreation:{}:{}", op, value)
            },
            NodesFilter::TimestampModification(op, value) => {
                let op = String::from(op);
                format!("timestampModification:{}:{}", op, value)
            },
            NodesFilter::ReferenceId(op, value) => {
                let op = String::from(op);
                format!("referenceId:{}:{}", op, value)
            },
        }
    }
}

impl NodesFilter {
    pub fn name_equals(val: &str) -> Self {
        NodesFilter::Name(FilterOperator::Eq, String::from(val))
    }

    pub fn name_contains(val: &str) -> Self {
        NodesFilter::Name(FilterOperator::Cn, String::from(val))
    }

    pub fn is_encrypted(val: bool) -> Self {
        NodesFilter::Encrypted(FilterOperator::Eq, val)
    }

    pub fn reference_id_equals(val: u64) -> Self {
        NodesFilter::ReferenceId(FilterOperator::Eq, val)
    }

    pub fn created_before(val: &str) -> Self {
        NodesFilter::TimestampCreation(FilterOperator::Le, String::from(val))
    }

    pub fn created_after(val: &str) -> Self {
        NodesFilter::TimestampCreation(FilterOperator::Ge, String::from(val))
    }

    pub fn modified_before(val: &str) -> Self {
        NodesFilter::TimestampModification(FilterOperator::Le, String::from(val))
    }

    pub fn modified_after(val: &str) -> Self {
        NodesFilter::TimestampModification(FilterOperator::Ge, String::from(val))
    }

    pub fn branch_version_before(val: u64) -> Self {
        NodesFilter::BranchVersion(FilterOperator::Le, val)
    }

    pub fn branch_version_after(val: u64) -> Self {
        NodesFilter::BranchVersion(FilterOperator::Ge, val)
    }

    pub fn is_file() -> Self {
        NodesFilter::Type(FilterOperator::Eq, NodeType::File)
    }

    pub fn is_folder() -> Self {
        NodesFilter::Type(FilterOperator::Eq, NodeType::Folder)
    }

    pub fn is_room() -> Self {
        NodesFilter::Type(FilterOperator::Eq, NodeType::Room)
    }
 
}

impl From<NodesFilter> for Box<dyn FilterQuery> {
    fn from(value: NodesFilter) -> Self {
        Box::new(value)  
    }
}

#[derive(Debug)]
pub enum NodesSearchFilter {
    Type(FilterOperator, NodeType),
    FileType(FilterOperator, String),
    Classification(FilterOperator, u8),
    CreatedBy(FilterOperator, String),
    UpdatedBy(FilterOperator, String),
    CreatedById(FilterOperator, u64),
    UpdatedById(FilterOperator, u64),
    CreatedAt(FilterOperator, String),
    UpdatedAt(FilterOperator, String),
    ExpireAt(FilterOperator, String),
    Size(FilterOperator, u64),
    IsFavorite(FilterOperator, bool),
    BranchVersion(FilterOperator, u64),
    ParentPath(FilterOperator, String),
    TimestampCreation(FilterOperator, String),
    TimestampModification(FilterOperator, String),
    ReferenceId(FilterOperator, u64),

}

impl NodesSearchFilter {
    pub fn is_file() -> Self {
        NodesSearchFilter::Type(FilterOperator::Eq, NodeType::File)
    }

    pub fn is_folder() -> Self {
        NodesSearchFilter::Type(FilterOperator::Eq, NodeType::Folder)
    }

    pub fn is_room() -> Self {
        NodesSearchFilter::Type(FilterOperator::Eq, NodeType::Room)
    }

    pub fn is_favorite(val: bool) -> Self {
        NodesSearchFilter::IsFavorite(FilterOperator::Eq, val)
    }

    pub fn parent_path_equals(val: &str) -> Self {
        NodesSearchFilter::ParentPath(FilterOperator::Eq, String::from(val))
    }

    pub fn parent_path_contains(val: &str) -> Self {
        NodesSearchFilter::ParentPath(FilterOperator::Cn, String::from(val))
    }

    pub fn size_greater_equals(val: u64) -> Self {
        NodesSearchFilter::Size(FilterOperator::Ge, val)
    }

    pub fn size_less_equals(val: u64) -> Self {
        NodesSearchFilter::Size(FilterOperator::Le, val)
    }

    pub fn branch_version_before(val: u64) -> Self {
        NodesSearchFilter::BranchVersion(FilterOperator::Le, val)
    }

    pub fn branch_version_after(val: u64) -> Self {
        NodesSearchFilter::BranchVersion(FilterOperator::Ge, val)
    }

    pub fn created_at_before(val: &str) -> Self {
        NodesSearchFilter::CreatedAt(FilterOperator::Le, String::from(val))
    }

    pub fn created_at_after(val: &str) -> Self {
        NodesSearchFilter::CreatedAt(FilterOperator::Ge, String::from(val))
    }

    pub fn updated_at_before(val: &str) -> Self {
        NodesSearchFilter::UpdatedAt(FilterOperator::Le, String::from(val))
    }

    pub fn updated_at_after(val: &str) -> Self {
        NodesSearchFilter::UpdatedAt(FilterOperator::Ge, String::from(val))
    }

    pub fn expire_at_before(val: &str) -> Self {
        NodesSearchFilter::ExpireAt(FilterOperator::Le, String::from(val))
    }

    pub fn expire_at_after(val: &str) -> Self {
        NodesSearchFilter::ExpireAt(FilterOperator::Ge, String::from(val))
    }

    pub fn classification_equals(val: u8) -> Self {
        NodesSearchFilter::Classification(FilterOperator::Eq, val)
    }

    pub fn file_type_equals(val: &str) -> Self {
        NodesSearchFilter::FileType(FilterOperator::Eq, String::from(val))
    }

    pub fn file_type_contains(val: &str) -> Self {
        NodesSearchFilter::FileType(FilterOperator::Cn, String::from(val))
    }

}

impl From<NodesSearchFilter> for Box<dyn FilterQuery> {
    fn from(value: NodesSearchFilter) -> Self {
        Box::new(value)  
    }
}

impl FilterQuery for NodesSearchFilter {

    fn filter_to_string(&self) -> String {
        match self {
            NodesSearchFilter::BranchVersion(op, val) => {
                let op = String::from(op);
                format!("branchVersion:{}:{}", op, val)
            },
            NodesSearchFilter::Type(op, val) => {
                let op = String::from(op);
                let node_type: String = val.into();
                format!("type:{}:{}", op, node_type)
            },
            NodesSearchFilter::FileType(op, val) => {
                let op = String::from(op);
                format!("fileType:{}:{}", op, val)
            },
            NodesSearchFilter::Classification(op, val) => {
                let op = String::from(op);
                format!("classification:{}:{}", op, val)
            },
            NodesSearchFilter::CreatedBy(op, val) => {
                let op = String::from(op);
                format!("createdBy:{}:{}", op, val)
            },
            NodesSearchFilter::UpdatedBy(op, val) => {
                let op = String::from(op);
                format!("updatedBy:{}:{}", op, val)
            },
            NodesSearchFilter::CreatedById(op, val) => {
                let op = String::from(op);
                format!("createdById:{}:{}", op, val)
            },
            NodesSearchFilter::UpdatedById(op, val) => {
                let op = String::from(op);
                format!("updatedById:{}:{}", op, val)
            },
            NodesSearchFilter::CreatedAt(op, val) => {
                let op = String::from(op);
                format!("createdAt:{}:{}", op, val)
            },
            NodesSearchFilter::UpdatedAt(op, val) => {
                let op = String::from(op);
                format!("updatedAt:{}:{}", op, val)
            },
            NodesSearchFilter::ExpireAt(op, val) => {
                let op = String::from(op);
                format!("expireAt:{}:{}", op, val)
            },
            NodesSearchFilter::Size(op, val) => {
                let op = String::from(op);
                format!("size:{}:{}", op, val)
            },
            NodesSearchFilter::IsFavorite(op, val) => {
                let op = String::from(op);
                format!("isFavorite:{}:{}", op, val)
            },
            NodesSearchFilter::ParentPath(op, val) => {
                let op = String::from(op);
                format!("parentPath:{}:{}", op, val)
            },
            NodesSearchFilter::TimestampCreation(op, val) => {
                let op = String::from(op);
                format!("timestampCreation:{}:{}", op, val)
            },
            NodesSearchFilter::TimestampModification(op, val) => {
                let op = String::from(op);
                format!("timestampModification:{}:{}", op, val)
            },
            NodesSearchFilter::ReferenceId(op, val) => {
                let op = String::from(op);
                format!("referenceId:{}:{}", op, val)
            },
        }
    }
}


#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_nodes_filter_name_eq() {
        let filter = NodesFilter::name_equals("test");
        assert_eq!(filter.filter_to_string(), "name:eq:test");
    }

    #[test]
    fn test_nodes_filter_name_contains() {
        let filter = NodesFilter::name_contains("test");
        assert_eq!(filter.filter_to_string(), "name:cn:test");
    }

    #[test]
    fn test_filter_branch_version_before() {
        let filter = NodesFilter::branch_version_before(1);
        assert_eq!(filter.filter_to_string(), "branchVersion:le:1");
    }

    #[test]
    fn test_nodes_filter_branch_version_after() {
        let filter = NodesFilter::branch_version_after(1);
        assert_eq!(filter.filter_to_string(), "branchVersion:ge:1");
    }

    #[test]
    fn test_nodes_filter_is_file() {
        let filter = NodesFilter::is_file();
        assert_eq!(filter.filter_to_string(), "type:eq:file");
    }

}