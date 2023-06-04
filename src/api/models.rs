use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ListAllParams {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
    pub filter: Option<String>,
    pub sort: Option<String>,
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

    pub fn with_filter(mut self, filter: String) -> Self {
        self.params.filter = Some(filter);
        self
    }

    pub fn with_sort(mut self, sort: String) -> Self {
        self.params.sort = Some(sort);
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

        let params = value
            .filter
            .map(|filter| format!("{params}&filter={filter}"))
            .unwrap_or(params);
        let params = value
            .sort
            .map(|sort| format!("{params}&sort={sort}"))
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


impl<'a, T> Iterator for &'a RangedItems<T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.iter().next()
    }
}

impl<T> Iterator for RangedItems<T> where T: Clone {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.iter().next().cloned()
    }
}