use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ListAllParams {
    offset: Option<u64>,
    limit: Option<u64>,
    filter: Option<String>,
    sort: Option<String>,
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

impl From<ListAllParams> for String {
    fn from(value: ListAllParams) -> Self {
        let params = format!("?offset={}", value.offset.unwrap_or(0));

        let params = value
            .filter
            .map(|filter| format!("{}&filter={}", params, filter))
            .unwrap_or(params);
        let params = value
            .sort
            .map(|sort| format!("{}&sort={}", params, sort))
            .unwrap_or(params);

        value
            .limit
            .map(|limit| format!("{}&limit={}", params, limit))
            .unwrap_or(params)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Range {
    offset: u64,
    limit: u64,
    total: u64,
}
