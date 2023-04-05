use reqwest::Response;
use serde::de::DeserializeOwned;

use crate::api::auth::{errors::DracoonClientError, models::StatusCodeState};

pub mod strings;

pub async fn parse_body<T, E>(res: Response) -> Result<T, DracoonClientError>
where
    T: DeserializeOwned,
    E: DeserializeOwned + Into<DracoonClientError>,
{
    match Into::<StatusCodeState>::into(res.status()) {
        StatusCodeState::Ok(_) => Ok(res.json::<T>().await.unwrap()),
        StatusCodeState::Error(_) => Err(build_error_body::<E>(res.json::<E>().await?).await),
    }
}

async fn build_error_body<E>(body: E) -> DracoonClientError
where
    E: DeserializeOwned + Into<DracoonClientError>,
{
    body.into()
}
