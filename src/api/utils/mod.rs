use reqwest::Response;
use serde::de::DeserializeOwned;
use serde_xml_rs::from_str;

use super::{auth::{errors::DracoonClientError, models::StatusCodeState}, nodes::models::{S3ErrorResponse, S3XmlError}};

pub async fn parse_body<T, E>(res: Response) -> Result<T, DracoonClientError>
where
    T: DeserializeOwned,
    E: DeserializeOwned + Into<DracoonClientError>,
{
    match Into::<StatusCodeState>::into(res.status()) {
        StatusCodeState::Ok(_) => Ok(res.json::<T>().await.expect("Correct response type")),
        StatusCodeState::Error(_) => Err(build_error_body::<E>(res.json::<E>().await?).await),
    }
}

async fn build_error_body<E>(body: E) -> DracoonClientError
where
    E: DeserializeOwned + Into<DracoonClientError>,
{
    body.into()
}

async fn build_s3_error(response: Response) -> DracoonClientError {
    let status = &response.status();
    let text = response.text().await.expect("Valid S3 XML error");
    let error: S3XmlError = from_str(&text).expect("Valid S3 XML error");
    let err_response = S3ErrorResponse::from_xml_error(status.clone(), error);
    return DracoonClientError::S3Error(err_response);
}