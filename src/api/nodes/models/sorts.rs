use crate::api::models::{SortOrder, SortQuery};

#[derive(Debug)]
pub enum NodesSortBy {
    Name(SortOrder),
    CreatedAt(SortOrder),
    CreatedBy(SortOrder),
    UpdatedAt(SortOrder),
    UpdatedBy(SortOrder),
    FileType(SortOrder),
    Classification(SortOrder),
    Size(SortOrder),
    CntDeletedVersions(SortOrder),
    TimestampCreation(SortOrder),
    TimestampModification(SortOrder),
}

impl From<NodesSortBy> for String {
    fn from(sort_by: NodesSortBy) -> Self {
        match sort_by {
            NodesSortBy::Name(order) => {
                let order = String::from(order);
                format!("name:{}", order)
            },
            NodesSortBy::CreatedAt(order) => {
                let order = String::from(order);
                format!("createdAt:{}", order)
            },
            NodesSortBy::CreatedBy(order) => {
                let order = String::from(order);
                format!("createdBy:{}", order)
            },
            NodesSortBy::UpdatedAt(order) => {
                let order = String::from(order);
                format!("updatedAt:{}", order)
            },
            NodesSortBy::UpdatedBy(order) => {
                let order = String::from(order);
                format!("updatedBy:{}", order)
            },
            NodesSortBy::FileType(order) => {
                let order = String::from(order);
                format!("fileType:{}", order)
            },
            NodesSortBy::Classification(order) => {
                let order = String::from(order);
                format!("classification:{}", order)
            },
            NodesSortBy::Size(order) => {
                let order = String::from(order);
                format!("size:{}", order)
            },
            NodesSortBy::CntDeletedVersions(order) => {
                let order = String::from(order);
                format!("cntDeletedVersions:{}", order)
            },
            NodesSortBy::TimestampCreation(order) => {
                let order = String::from(order);
                format!("timestampCreation:{}", order)
            },
            NodesSortBy::TimestampModification(order) => {
                let order = String::from(order);
                format!("timestampModification:{}", order)
            },

        }
    }
}

#[derive(Debug)]
pub enum NodesSearchSortBy {
   Name(SortOrder),
   CreatedAt(SortOrder),
   CreatedBy(SortOrder),
   UpdatedAt(SortOrder),
   UpdatedBy(SortOrder),
   FileType(SortOrder),
   Classification(SortOrder),
   Size(SortOrder),
   CntDeletedVersions(SortOrder),
   Type(SortOrder),
   ParentPath(SortOrder),
   TimestampCreation(SortOrder),
   TimestampModification(SortOrder),   
}

impl From<NodesSearchSortBy> for String {
    fn from(value: NodesSearchSortBy) -> Self {
        match value {
            NodesSearchSortBy::Name(order) => {
                let order = String::from(order);
                format!("name:{}", order)
            },
            NodesSearchSortBy::CreatedAt(order) => {
                let order = String::from(order);
                format!("createdAt:{}", order)
            },
            NodesSearchSortBy::CreatedBy(order) => {
                let order = String::from(order);
                format!("createdBy:{}", order)
            },
            NodesSearchSortBy::UpdatedAt(order) => {
                let order = String::from(order);
                format!("updatedAt:{}", order)
            },
            NodesSearchSortBy::UpdatedBy(order) => {
                let order = String::from(order);
                format!("updatedBy:{}", order)
            },
            NodesSearchSortBy::FileType(order) => {
                let order = String::from(order);
                format!("fileType:{}", order)
            },
            NodesSearchSortBy::Classification(order) => {
                let order = String::from(order);
                format!("classification:{}", order)
            },
            NodesSearchSortBy::Size(order) => {
                let order = String::from(order);
                format!("size:{}", order)
            },
            NodesSearchSortBy::CntDeletedVersions(order) => {
                let order = String::from(order);
                format!("cntDeletedVersions:{}", order)
            },
            NodesSearchSortBy::Type(order) => {
                let order = String::from(order);
                format!("type:{}", order)
            },
            NodesSearchSortBy::ParentPath(order) => {
                let order = String::from(order);
                format!("parentPath:{}", order)
            },
            NodesSearchSortBy::TimestampCreation(order) => {
                let order = String::from(order);
                format!("timestampCreation:{}", order)
            },
            NodesSearchSortBy::TimestampModification(order) => {
                let order = String::from(order);
                format!("timestampModification:{}", order)
            },
            
        }
    }
}

impl NodesSearchSortBy {
    pub fn parent_path_asc() -> Self {
        NodesSearchSortBy::ParentPath(SortOrder::Asc)
    }

    pub fn parent_path_desc() -> Self {
        NodesSearchSortBy::ParentPath(SortOrder::Desc)
    }

    pub fn name_asc() -> Self {
        NodesSearchSortBy::Name(SortOrder::Asc)
    }

    pub fn name_desc() -> Self {
        NodesSearchSortBy::Name(SortOrder::Desc)
    }

    pub fn created_at_asc() -> Self {
        NodesSearchSortBy::CreatedAt(SortOrder::Asc)
    }

    pub fn created_at_desc() -> Self {
        NodesSearchSortBy::CreatedAt(SortOrder::Desc)
    }

    pub fn created_by_asc() -> Self {
        NodesSearchSortBy::CreatedBy(SortOrder::Asc)
    }

    pub fn created_by_desc() -> Self {
        NodesSearchSortBy::CreatedBy(SortOrder::Desc)
    }

    pub fn updated_at_asc() -> Self {
        NodesSearchSortBy::UpdatedAt(SortOrder::Asc)
    }

    pub fn updated_at_desc() -> Self {
        NodesSearchSortBy::UpdatedAt(SortOrder::Desc)
    }

    pub fn updated_by_asc() -> Self {
        NodesSearchSortBy::UpdatedBy(SortOrder::Asc)
    }

    pub fn updated_by_desc() -> Self {
        NodesSearchSortBy::UpdatedBy(SortOrder::Desc)
    }

    pub fn file_type_asc() -> Self {
        NodesSearchSortBy::FileType(SortOrder::Asc)
    }

    pub fn file_type_desc() -> Self {
        NodesSearchSortBy::FileType(SortOrder::Desc)
    }

    pub fn classification_asc() -> Self {
        NodesSearchSortBy::Classification(SortOrder::Asc)
    }

    pub fn classification_desc() -> Self {
        NodesSearchSortBy::Classification(SortOrder::Desc)
    }

    pub fn size_asc() -> Self {
        NodesSearchSortBy::Size(SortOrder::Asc)
    }

    pub fn size_desc() -> Self {
        NodesSearchSortBy::Size(SortOrder::Desc)
    }

    pub fn cnt_deleted_versions_asc() -> Self {
        NodesSearchSortBy::CntDeletedVersions(SortOrder::Asc)
    }

    pub fn cnt_deleted_versions_desc() -> Self {
        NodesSearchSortBy::CntDeletedVersions(SortOrder::Desc)
    }

    pub fn type_asc() -> Self {
        NodesSearchSortBy::Type(SortOrder::Asc)
    }

    pub fn type_desc() -> Self {
        NodesSearchSortBy::Type(SortOrder::Desc)
    }

    pub fn timestamp_creation_asc() -> Self {
        NodesSearchSortBy::TimestampCreation(SortOrder::Asc)
    }

    pub fn timestamp_creation_desc() -> Self {
        NodesSearchSortBy::TimestampCreation(SortOrder::Desc)
    }

    pub fn timestamp_modification_asc() -> Self {
        NodesSearchSortBy::TimestampModification(SortOrder::Asc)
    }

    pub fn timestamp_modification_desc() -> Self {
        NodesSearchSortBy::TimestampModification(SortOrder::Desc)
    }
    
}

impl SortQuery for NodesSearchSortBy {
    fn sort_to_string(&self) -> String {
        match self {
            NodesSearchSortBy::Name(order) => {
                let order = String::from(order);
                format!("name:{}", order)
            },
            NodesSearchSortBy::CreatedAt(order) => {
                let order = String::from(order);
                format!("createdAt:{}", order)
            },
            NodesSearchSortBy::CreatedBy(order) => {
                let order = String::from(order);
                format!("createdBy:{}", order)
            },
            NodesSearchSortBy::UpdatedAt(order) => {
                let order = String::from(order);
                format!("updatedAt:{}", order)
            },
            NodesSearchSortBy::UpdatedBy(order) => {
                let order = String::from(order);
                format!("updatedBy:{}", order)
            },
            NodesSearchSortBy::FileType(order) => {
                let order = String::from(order);
                format!("fileType:{}", order)
            },
            NodesSearchSortBy::Classification(order) => {
                let order = String::from(order);
                format!("classification:{}", order)
            },
            NodesSearchSortBy::Size(order) => {
                let order = String::from(order);
                format!("size:{}", order)
            },
            NodesSearchSortBy::CntDeletedVersions(order) => {
                let order = String::from(order);
                format!("cntDeletedVersions:{}", order)
            },
            NodesSearchSortBy::Type(order) => {
                let order = String::from(order);
                format!("type:{}", order)
            },
            NodesSearchSortBy::ParentPath(order) => {
                let order = String::from(order);
                format!("parentPath:{}", order)
            },
            NodesSearchSortBy::TimestampCreation(order) => {
                let order = String::from(order);
                format!("timestampCreation:{}", order)
            },
            NodesSearchSortBy::TimestampModification(order) => {
                let order = String::from(order);
                format!("timestampModification:{}", order)
            },
        }
    }
}

impl From<NodesSearchSortBy> for Box<dyn SortQuery> {
    fn from(value: NodesSearchSortBy) -> Self {
        Box::new(value)  
    }
}

impl From<NodesSortBy> for Box<dyn SortQuery> {
    fn from(value: NodesSortBy) -> Self {
        Box::new(value)  
    }
}

impl SortQuery for NodesSortBy {
    fn sort_to_string(&self) -> String {
        match self {
            NodesSortBy::Classification(order) => {
                let order = String::from(order);
                format!("classification:{}", order)
            },
            NodesSortBy::CreatedAt(order) => {
                let order = String::from(order);
                format!("createdAt:{}", order)
            },
            NodesSortBy::CreatedBy(order) => {
                let order = String::from(order);
                format!("createdBy:{}", order)
            },
            NodesSortBy::FileType(order) => {
                let order = String::from(order);
                format!("fileType:{}", order)
            },
            NodesSortBy::Name(order) => {
                let order = String::from(order);
                format!("name:{}", order)
            },
            NodesSortBy::Size(order) => {
                let order = String::from(order);
                format!("size:{}", order)
            },
            NodesSortBy::UpdatedAt(order) => {
                let order = String::from(order);
                format!("updatedAt:{}", order)
            },
            NodesSortBy::UpdatedBy(order) => {
                let order = String::from(order);
                format!("updatedBy:{}", order)
            },
            NodesSortBy::CntDeletedVersions(order) => {
                let order = String::from(order);
                format!("cntDeletedVersions:{}", order)
            },
            NodesSortBy::TimestampCreation(order) => {
                let order = String::from(order);
                format!("timestampCreation:{}", order)
            },
            NodesSortBy::TimestampModification(order) => {
                let order = String::from(order);
                format!("timestampModification:{}", order)
            },


        }
    }
}