use std::fmt::Debug;

use serde::{Deserialize, Serialize};


#[derive(Debug)]
pub struct ListAllParams {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
    pub filter: Option<FilterQueries>,
    pub sort: Option<SortQueries>,
}

impl Default for ListAllParams {
    fn default() -> Self {
        Self {
            offset: Some(0),
            limit: None,
            filter: None,
            sort: None,
        }
    }
}

impl ListAllParams {
    pub fn builder() -> ListAllParamsBuilder {
        ListAllParamsBuilder::new()
    }

    pub fn filter_to_string(&self) -> String {

        match self.filter.as_deref() {
            Some(filters) =>{
                filters
                    .iter()
                    .map(|filter| filter.filter_to_string())
                    .collect::<Vec<String>>()
                    .join("|")
            },
            None => String::new(),
            }
        }

    pub fn sort_to_string(&self) -> String {

        match self.sort.as_deref() {
            Some(sorts) =>{
                sorts
                    .into_iter()
                    .map(|sort| sort.sort_to_string())
                    .collect::<Vec<String>>()
                    .join("|")
            },
            None => String::new(),
            }
        }
}

pub struct ListAllParamsBuilder {
    params: ListAllParams,
}

impl ListAllParamsBuilder {
    pub fn new() -> Self {
        Self {
            params: ListAllParams::default(),
        }
    }
    pub fn with_offset(mut self, offset: u64) -> Self {
        self.params.offset = Some(offset);
        self
    }

    pub fn with_limit(mut self, limit: u64) -> Self {
        self.params.limit = Some(limit);
        self
    }

    pub fn with_filter<F>(mut self, filter: F) -> Self where F: Into<Box<dyn FilterQuery>>  {
        
        match self.params.filter {
            Some(mut filters) => {
                filters.push(filter.into());
                self.params.filter = Some(filters);
            },
            None => {
                let mut filters = Vec::new();
                filters.push(filter.into());
                self.params.filter = Some(filters);
            }
        }
        self
    }

    pub fn with_sort<S>(mut self, sort: S) -> Self where S: Into<Box<dyn SortQuery>>  {

        match self.params.sort {
            Some(mut sorts) => {
                sorts.push(sort.into());
                self.params.sort = Some(sorts);
            },
            None => {
                let mut sorts = Vec::new();
                sorts.push(sort.into());
                self.params.sort = Some(sorts);
            }
        }

        self
    }

    pub fn build(self) -> ListAllParams {
        self.params
    }
}

impl Default for ListAllParamsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ListAllParams> for String {
    fn from(value: ListAllParams) -> Self {
        let params = format!("?offset={}", value.offset.unwrap_or(0));

        let filters = value.filter_to_string();
        let sorts = value.sort_to_string();

        let params = value
            .filter
            .map(|filter| format!("{params}&filter={}", filters))
            .unwrap_or(params);
        let params = value
            .sort
            .map(|sort| format!("{params}&sort={}", sorts))
            .unwrap_or(params);

        value
            .limit
            .map(|limit| format!("{params}&limit={limit}"))
            .unwrap_or(params)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Range {
    pub offset: u64,
    pub limit: u64,
    pub total: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ObjectExpiration {
    pub enable_expiration: bool,
    pub expire_at: Option<String>,
}

impl AsRef<ObjectExpiration> for ObjectExpiration {
    fn as_ref(&self) -> &Self {
        self
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct RangedItems<T> {
    pub range: Range,
    pub items: Vec<T>,
}

impl <T> IntoIterator for RangedItems<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

pub trait FilterQuery: Debug + Send + Sync {
    fn filter_to_string(&self) -> String;
}

pub trait SortQuery: Debug + Send + Sync {
    fn sort_to_string(&self) -> String;
}

pub type FilterQueries = Vec<Box<dyn FilterQuery>>;
pub type SortQueries = Vec<Box<dyn SortQuery>>;

#[derive(Debug)]
pub enum FilterOperator {
    Eq,
    Cn,
    Neq,
    Ge,
    Le,
}

impl From<FilterOperator> for String {
    fn from(value: FilterOperator) -> Self {
        match value {
            FilterOperator::Eq => "eq".to_string(),
            FilterOperator::Cn => "cn".to_string(),
            FilterOperator::Neq => "neq".to_string(),
            FilterOperator::Ge => "ge".to_string(),
            FilterOperator::Le => "le".to_string(),
        }
    }
}

impl From<&FilterOperator> for String {
    fn from(value: &FilterOperator) -> Self {
        match value {
            FilterOperator::Eq => "eq".to_string(),
            FilterOperator::Cn => "cn".to_string(),
            FilterOperator::Neq => "neq".to_string(),
            FilterOperator::Ge => "ge".to_string(),
            FilterOperator::Le => "le".to_string(),
        }
    }
}

#[derive(Debug)]
pub enum SortOrder {
    Asc,
    Desc,
}

impl From<SortOrder> for String {
    fn from(value: SortOrder) -> Self {
        match value {
            SortOrder::Asc => "asc".to_string(),
            SortOrder::Desc => "desc".to_string(),
        }
    }
}

impl From<&SortOrder> for String {
    fn from(value: &SortOrder) -> Self {
        match value {
            SortOrder::Asc => "asc".to_string(),
            SortOrder::Desc => "desc".to_string(),
        }
    }
}