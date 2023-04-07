use std::io::Read;
use async_trait::async_trait;
use crate::api::auth::errors::DracoonClientError;
use super::{models::{Node, ProgressCallback, FileMeta, CreateFileUploadResponse, PresignedUrlList, S3FileUploadStatus}, Upload};

#[async_trait]
impl <C: UploadInternal + Sync> Upload for C {

    async fn upload<'r>(&'r self, file_meta: FileMeta, parent_node: &Node, reader: &'r mut (dyn Read + Send), callback: Option<ProgressCallback>) -> Result<(), DracoonClientError> {
        match parent_node.is_encrypted {
            Some(encrypted) => {
                if encrypted {
                    self.upload_to_s3_encrypted(
                        file_meta,
                        parent_node,
                        reader,
                        callback,
                    ).await
                } else {
                    self.upload_to_s3_unencrypted(
                        file_meta,
                        parent_node,
                        reader,
                        callback,
                    ).await
                }
            }
            None => {
                self.upload_to_s3_unencrypted(
                    file_meta,
                    parent_node,
                    reader,
                    callback,
                ).await
            }
        }
    }

}

#[async_trait]
trait UploadInternal {
    async fn create_upload_channel(
        &self,
        parent_node: &Node,
    ) -> Result<CreateFileUploadResponse, DracoonClientError>;

    async fn get_s3_upload_urls(&self, upload_id: u64)
        -> Result<PresignedUrlList, DracoonClientError>;

    async fn upload_to_s3_unencrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        reader: &mut (dyn Read + Send),
        mut callback: Option<ProgressCallback>,
    ) -> Result<(), DracoonClientError>;
    async fn upload_to_s3_encrypted(
        &self,
        file_meta: FileMeta,
        parent_node: &Node,
        reader: &mut (dyn Read + Send),
        mut callback: Option<ProgressCallback>,
    ) -> Result<(), DracoonClientError>;

    async fn finalize_upload(&self, upload_id: u64) -> Result<(), DracoonClientError>;

    async fn get_upload_status(
        &self,
        upload_id: u64,
    ) -> Result<S3FileUploadStatus, DracoonClientError>;
}
