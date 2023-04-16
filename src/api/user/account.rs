use async_trait::async_trait;
use reqwest::header;

use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{DRACOON_API_PREFIX, USER_ACCOUNT, USER_BASE},
    utils::FromResponse,
    Dracoon,
};

use super::{
    models::{UpdateUserAccountRequest, UserAccount},
    User,
};

#[async_trait]
impl User for Dracoon<Connected> {
    async fn get_user_account(&self) -> Result<UserAccount, DracoonClientError> {
        let url_part = format!("{}/{}/{}", DRACOON_API_PREFIX, USER_BASE, USER_ACCOUNT);

        let url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .get(url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        UserAccount::from_response(response).await
    }
    async fn update_user_account(
        &self,
        update: UpdateUserAccountRequest,
    ) -> Result<UserAccount, DracoonClientError> {
        let url_part = format!("{}/{}/{}", DRACOON_API_PREFIX, USER_BASE, USER_ACCOUNT);

        let url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .put(url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&update)
            .send()
            .await?;

        UserAccount::from_response(response).await
    }
}
