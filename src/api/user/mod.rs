use async_trait::async_trait;
use dco3_crypto::{PlainUserKeyPairContainer};

use self::models::{UserAccount, UpdateUserAccountRequest};
use super::auth::errors::DracoonClientError;

pub mod models;
pub mod account;
pub mod keypairs;


#[async_trait]
pub trait User {
    async fn get_user_account(&self) -> Result<UserAccount, DracoonClientError>;
    async fn update_user_account(&self, update: UpdateUserAccountRequest) -> Result<UserAccount, DracoonClientError>;
}

#[async_trait]
pub trait UserAccountKeypairs {
    async fn get_user_keypair(&self, secret: &str) -> Result<PlainUserKeyPairContainer, DracoonClientError>;
    async fn set_user_keypair(&self, secret: &str) -> Result<(), DracoonClientError>;
    async fn delete_user_keypair(&self) -> Result<(), DracoonClientError>;
}