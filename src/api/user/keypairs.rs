use async_trait::async_trait;
use dco3_crypto::{PlainUserKeyPairContainer, UserKeyPairContainer, DracoonCrypto, DracoonRSACrypto};
use crate::api::{auth::{Connected, errors::DracoonClientError}, Dracoon, constants::{DRACOON_API_PREFIX, USER_BASE, USER_ACCOUNT, USER_ACCOUNT_KEYPAIR}, utils::FromResponse};
use super::UserAccountKeypairs;


#[async_trait]
impl UserAccountKeypairs for Dracoon<Connected> {
    async fn get_user_keypair(&self, secret: &str) -> Result<PlainUserKeyPairContainer, DracoonClientError> {
        let url_part = format!("{}/{}/{}/{}", DRACOON_API_PREFIX, USER_BASE, USER_ACCOUNT, USER_ACCOUNT_KEYPAIR);

        let url = self.build_api_url(&url_part);

        let response = self.client.http.get(url).send().await?;

        let enc_keypair = UserKeyPairContainer::from_response(response).await?;
        let plain_keypair = DracoonCrypto::decrypt_private_key(secret, enc_keypair)?;

        Ok(plain_keypair)

    }

    async fn set_user_keypair(&self, secret: &str) -> Result<(), DracoonClientError> {
        let url_part = format!("{}/{}/{}/{}", DRACOON_API_PREFIX, USER_BASE, USER_ACCOUNT, USER_ACCOUNT_KEYPAIR);

        let url = self.build_api_url(&url_part);

        let version = dco3_crypto::UserKeyPairVersion::RSA4096;
        let keypair = DracoonCrypto::create_plain_user_keypair(version)?;
        let enc_keypair = DracoonCrypto::encrypt_private_key(secret, keypair)?;

        let response = self.client.http.post(url).json(&enc_keypair).send().await?;

        Ok(())
    }

    async fn delete_user_keypair(&self) -> Result<(), DracoonClientError> {
        let url_part = format!("{}/{}/{}/{}", DRACOON_API_PREFIX, USER_BASE, USER_ACCOUNT, USER_ACCOUNT_KEYPAIR);

        let url = self.build_api_url(&url_part);

        let response = self.client.http.delete(url).send().await?;

        Ok(())
    }
    
}