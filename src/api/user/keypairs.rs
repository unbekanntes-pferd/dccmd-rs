use super::UserAccountKeypairs;
use crate::api::{
    auth::{errors::DracoonClientError, Connected},
    constants::{DRACOON_API_PREFIX, USER_ACCOUNT, USER_ACCOUNT_KEYPAIR, USER_BASE},
    utils::FromResponse,
    Dracoon,
};
use async_trait::async_trait;
use dco3_crypto::{
    DracoonCrypto, DracoonRSACrypto, PlainUserKeyPairContainer, UserKeyPairContainer,
};
use reqwest::header;

#[async_trait]
impl UserAccountKeypairs for Dracoon<Connected> {
    async fn get_user_keypair(
        &self,
        secret: &str,
    ) -> Result<PlainUserKeyPairContainer, DracoonClientError> {
        let url_part = format!(
            "{DRACOON_API_PREFIX}/{USER_BASE}/{USER_ACCOUNT}/{USER_ACCOUNT_KEYPAIR}"
        );

        let url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .get(url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        let enc_keypair = UserKeyPairContainer::from_response(response).await?;
        let plain_keypair = DracoonCrypto::decrypt_private_key(secret, enc_keypair)?;

        Ok(plain_keypair)
    }

    async fn set_user_keypair(&self, secret: &str) -> Result<(), DracoonClientError> {
        let url_part = format!(
            "{DRACOON_API_PREFIX}/{USER_BASE}/{USER_ACCOUNT}/{USER_ACCOUNT_KEYPAIR}"
        );

        let url = self.build_api_url(&url_part);

        let version = dco3_crypto::UserKeyPairVersion::RSA4096;
        let keypair = DracoonCrypto::create_plain_user_keypair(version)?;
        let enc_keypair = DracoonCrypto::encrypt_private_key(secret, keypair)?;

        let response = self
            .client
            .http
            .post(url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&enc_keypair)
            .send()
            .await?;

        Ok(())
    }

    async fn delete_user_keypair(&self) -> Result<(), DracoonClientError> {
        let url_part = format!(
            "{DRACOON_API_PREFIX}/{USER_BASE}/{USER_ACCOUNT}/{USER_ACCOUNT_KEYPAIR}"
        );

        let url = self.build_api_url(&url_part);

        let response = self
            .client
            .http
            .delete(url)
            .header(header::AUTHORIZATION, self.get_auth_header().await?)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        Ok(())
    }
}
