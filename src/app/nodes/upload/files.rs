use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
};

use dco3::{
    auth::Connected,
    nodes::{Node, ResolutionStrategy, UploadOptions},
    Dracoon, Public, PublicUpload, Upload,
};
use futures_util::{stream, StreamExt};
use tracing::{debug, error, info, warn};

use crate::{
    app::{
        auth::AuthService,
        nodes::{
            command::CmdUploadOptions,
            progress::{start_progress_bar, update_remaining_files_message, ProgressReporter},
            upload::UploadOutcome,
        },
        shares::DownloadShareLinkCreator,
    },
    core::{
        constants::{
            DEFAULT_CHUNK_SIZE, DEFAULT_CONCURRENT_MULTIPLIER, MAX_VELOCITY, MIN_VELOCITY,
        },
        models::DcCmdError,
    },
};

use super::api::{get_file_meta, UploadApi, UploadProgressFn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadFailure {
    pub file: String,
    pub reason: String,
}

pub async fn upload_public_file(
    source: PathBuf,
    target: String,
    progress: &dyn ProgressReporter,
) -> Result<(), DcCmdError> {
    let file = tokio::fs::File::open(&source).await.map_err(|err| {
        error!("Error opening file: {}", err);
        DcCmdError::IoError
    })?;

    let file_meta = file.metadata().await.map_err(|err| {
        error!("Error getting file metadata for {:?}: {}", file, err);
        DcCmdError::IoError
    })?;

    if !file_meta.is_file() {
        return Err(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ));
    }

    let dracoon = AuthService::new().init_public(&target).await?;

    let access_key = target
        .split('/')
        .next_back()
        .ok_or(DcCmdError::InvalidPath(target.clone()))?;

    let upload_share = dracoon.public().get_public_upload_share(access_key).await?;

    let file_meta = get_file_meta(&file_meta, &source)?;

    let file_size = file_meta.size;

    let progress_bar = start_progress_bar(progress, file_size, Some("Uploading"));

    let upload_opts = UploadOptions::builder(file_meta).build();
    let buffer_size = calculate_buffer_size(file_size);
    let reader = tokio::io::BufReader::with_capacity(buffer_size, file);

    let progress_bar_mv = progress_bar.clone();

    dracoon
        .public()
        .upload(
            access_key,
            upload_share,
            upload_opts,
            reader,
            Some(Box::new(move |progress, _| {
                progress_bar_mv.inc(progress);
            })),
            Some(DEFAULT_CHUNK_SIZE),
        )
        .await?;

    Ok(())
}

pub async fn upload_file(
    dracoon: &Dracoon<Connected>,
    source: PathBuf,
    target_node: &Node,
    opts: CmdUploadOptions,
    share_link_creator: &dyn DownloadShareLinkCreator,
    progress: &dyn ProgressReporter,
) -> Result<Option<String>, DcCmdError> {
    info!("Attempting upload of file: {}.", source.to_string_lossy());
    info!("Target node: {}.", target_node.name);
    let file = tokio::fs::File::open(&source).await.map_err(|err| {
        error!("Error opening file: {}", err);
        DcCmdError::IoError
    })?;

    let file_meta = file.metadata().await.map_err(|err| {
        error!("Error getting file metadata for {:?}: {}", file, err);
        DcCmdError::IoError
    })?;

    if !file_meta.is_file() {
        return Err(DcCmdError::InvalidPath(
            source.to_string_lossy().to_string(),
        ));
    }

    let file_meta = get_file_meta(&file_meta, &source)?;
    let file_name = file_meta.name.clone();

    let progress_bar = start_progress_bar(progress, file_meta.size, Some("Uploading"));

    let progress_bar_mv = progress_bar.clone();

    let classification = opts.classification.unwrap_or(2);
    let resolution_strategy = if opts.overwrite {
        ResolutionStrategy::Overwrite
    } else {
        ResolutionStrategy::AutoRename
    };

    // only keep share links if overwrite is set
    let keep_share_links = match resolution_strategy {
        ResolutionStrategy::Overwrite => opts.keep_share_links,
        _ => false,
    };

    let upload_options = UploadOptions::builder(file_meta)
        .with_classification(classification)
        .with_resolution_strategy(resolution_strategy)
        .with_keep_share_links(keep_share_links)
        .build();

    let reader = tokio::io::BufReader::new(file);

    let node = dracoon
        .upload(
            target_node,
            upload_options,
            reader,
            Some(Box::new(move |progress, _| {
                progress_bar_mv.inc(progress);
            })),
            Some(DEFAULT_CHUNK_SIZE),
        )
        .await?;

    progress_bar.finish_with_message(&format!("Upload of {file_name} complete"));
    info!("Upload of {} complete.", source.to_string_lossy());

    let share_message = maybe_build_share_message(
        &node,
        &file_name,
        opts.share,
        opts.share_password,
        share_link_creator,
    )
    .await?;

    Ok(share_message)
}

async fn maybe_build_share_message(
    node: &Node,
    file_name: &str,
    share_enabled: bool,
    share_password: Option<String>,
    share_link_creator: &dyn DownloadShareLinkCreator,
) -> Result<Option<String>, DcCmdError> {
    if !share_enabled || node.is_encrypted.unwrap_or(false) {
        return Ok(None);
    }

    let link = share_link_creator
        .create_download_share_link(node, share_password)
        .await?;
    Ok(Some(format!("Shared {file_name}.\n▶︎▶︎ {link}")))
}

pub async fn upload_files<A: UploadApi + Clone + Send + Sync + 'static>(
    api: &A,
    parent_node: &Node,
    files: BTreeMap<PathBuf, (u64, u64)>,
    parent_nodes: HashMap<u64, Node>,
    opts: CmdUploadOptions,
    progress: &dyn ProgressReporter,
) -> Result<UploadOutcome, DcCmdError> {
    info!("Attempting upload of {} files.", files.len());

    let concurrent_reqs = concurrent_requests_for_velocity(opts.velocity);

    let total_size = files.values().map(|(_, size)| size).sum::<u64>();
    let classification = opts.classification.unwrap_or(2);
    let overwrite = opts.overwrite;
    let keep_share_links_flag = opts.keep_share_links;

    let count_files = files.len();
    let message = format!("Uploading {count_files} files");
    let progress_bar = start_progress_bar(progress, total_size, Some(&message));
    let remaining_files = Arc::new(AtomicU64::new(files.len() as u64));
    let uploaded_files = Arc::new(AtomicUsize::new(0));
    let parent_nodes = Arc::new(parent_nodes);

    let files_iter: Vec<_> = files.into_iter().collect();

    let task_results = stream::iter(files_iter.into_iter().map(|(source, (node_id, _))| {
        let api_client = api.clone();
        let progress_bar_mv = progress_bar.clone();
        let progress_bar_inc = progress_bar.clone();
        let remaining_files = remaining_files.clone();
        let uploaded_files = uploaded_files.clone();
        let parent_nodes = parent_nodes.clone();

        async move {
            let source_display = source.to_string_lossy().to_string();
            debug!("Uploading file: {source_display}");

            let result = async {
                let parent_node = parent_nodes.get(&node_id).cloned().ok_or_else(|| {
                    DcCmdError::InvalidPath(format!(
                        "Parent node not found in upload cache: {node_id}"
                    ))
                })?;

                let on_progress: UploadProgressFn = Arc::new(move |progress| {
                    progress_bar_mv.inc(progress);
                });

                api_client
                    .upload_local_file(
                        source,
                        &parent_node,
                        classification,
                        overwrite,
                        keep_share_links_flag,
                        Some(on_progress),
                    )
                    .await
            }
            .await;

            let remaining = remaining_files
                .fetch_sub(1, Ordering::Relaxed)
                .saturating_sub(1);
            update_remaining_files_message(progress_bar_inc.as_ref(), "Uploading", remaining);

            match result {
                Ok(file_name) => {
                    uploaded_files.fetch_add(1, Ordering::Relaxed);
                    debug!("Uploaded file: {}", file_name);
                    None
                }
                Err(err) => {
                    error!("Error uploading file: {source_display} ({err})");
                    Some(UploadFailure {
                        file: source_display,
                        reason: upload_error_reason(&err),
                    })
                }
            }
        }
    }))
    .buffer_unordered(concurrent_reqs)
    .collect::<Vec<Option<UploadFailure>>>()
    .await;

    let target = parent_node.name.clone();
    let uploaded_files = uploaded_files.load(Ordering::Relaxed);
    let failures = task_results.into_iter().flatten().collect::<Vec<_>>();
    let requested_total = count_files as u64;

    progress_bar.finish_with_message(&format!(
        "Upload to {target} complete ({uploaded_files}/{requested_total})"
    ));

    info!("Upload of {uploaded_files} files to {target} complete.");

    if !failures.is_empty() {
        warn!(
            "Failed to upload {} files to {target}.",
            requested_total.saturating_sub(uploaded_files as u64)
        );
        return Ok(UploadOutcome::from_failures(
            target,
            requested_total,
            uploaded_files as u64,
            failures,
        ));
    }

    Ok(UploadOutcome::success(target, requested_total))
}

fn calculate_buffer_size(file_size: u64) -> usize {
    const MEGABYTE: u64 = 1024 * 1024;
    match file_size {
        0..=MEGABYTE => 16 * 1024,
        size if size <= 10 * MEGABYTE => 64 * 1024,
        size if size <= 100 * MEGABYTE => 128 * 1024,
        _ => 256 * 1024,
    }
}

fn effective_velocity(velocity: Option<u8>) -> u8 {
    velocity
        .unwrap_or(MIN_VELOCITY)
        .clamp(MIN_VELOCITY, MAX_VELOCITY)
}

fn concurrent_requests_for_velocity(velocity: Option<u8>) -> usize {
    effective_velocity(velocity) as usize * DEFAULT_CONCURRENT_MULTIPLIER as usize
}

fn upload_error_reason(error: &DcCmdError) -> String {
    match error {
        DcCmdError::InvalidArgument(msg)
        | DcCmdError::InvalidPath(msg)
        | DcCmdError::InvalidUrl(msg) => msg.clone(),
        _ => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap},
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use async_trait::async_trait;
    use dco3::nodes::{models::NodeType, Node};
    use mockito::Server;

    use crate::{
        app::{
            nodes::{
                command::CmdUploadOptions, progress::NoopProgressReporter, upload::UploadOutcome,
            },
            shares::DownloadShareLinkCreator,
        },
        core::models::DcCmdError,
    };

    use super::{
        super::api::{UploadApi, UploadProgressFn},
        concurrent_requests_for_velocity, effective_velocity, maybe_build_share_message,
        upload_error_reason, upload_files, upload_public_file, UploadFailure,
    };

    fn unique_test_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }

    async fn simulate_bounded_tasks(task_count: usize, limit: usize) -> usize {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(limit));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak_in_flight = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..task_count {
            let semaphore = semaphore.clone();
            let in_flight = in_flight.clone();
            let peak_in_flight = peak_in_flight.clone();
            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.expect("semaphore permit");
                let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                peak_in_flight.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                in_flight.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for handle in handles {
            handle.await.expect("join handle");
        }

        peak_in_flight.load(Ordering::SeqCst)
    }

    #[derive(Default)]
    struct MockShareLinkCreator {
        calls: AtomicUsize,
    }

    #[derive(Clone, Default)]
    struct MockUploadApi {
        results: Arc<Mutex<HashMap<String, Result<String, DcCmdError>>>>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl MockUploadApi {
        fn with_results(results: Vec<(String, Result<String, DcCmdError>)>) -> Self {
            Self {
                results: Arc::new(Mutex::new(results.into_iter().collect())),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("lock poisoned").clone()
        }
    }

    #[async_trait]
    impl UploadApi for MockUploadApi {
        async fn upload_local_file(
            &self,
            source: PathBuf,
            _target_node: &Node,
            _classification: u8,
            _overwrite: bool,
            _keep_share_links: bool,
            on_progress: Option<UploadProgressFn>,
        ) -> Result<String, DcCmdError> {
            let source_display = source.to_string_lossy().to_string();
            self.calls
                .lock()
                .expect("lock poisoned")
                .push(source_display.clone());

            if let Some(on_progress) = on_progress {
                on_progress(1);
            }

            self.results
                .lock()
                .expect("lock poisoned")
                .remove(&source_display)
                .unwrap_or_else(|| {
                    Ok(source
                        .file_name()
                        .expect("file name")
                        .to_string_lossy()
                        .to_string())
                })
        }
    }

    #[async_trait]
    impl DownloadShareLinkCreator for MockShareLinkCreator {
        async fn create_download_share_link(
            &self,
            _node: &Node,
            _share_password: Option<String>,
        ) -> Result<String, DcCmdError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok("https://example.com/public/download-shares/mock-link".to_string())
        }
    }

    fn file_node(id: u64, encrypted: Option<bool>) -> Node {
        Node {
            id,
            reference_id: None,
            node_type: NodeType::File,
            name: "file.txt".to_string(),
            timestamp_creation: None,
            timestamp_modification: None,
            parent_id: None,
            parent_path: Some("/room/root/".to_string()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            expire_at: None,
            hash: None,
            file_type: None,
            media_type: None,
            size: Some(4),
            classification: None,
            notes: None,
            permissions: None,
            inherit_permissions: None,
            is_encrypted: encrypted,
            encryption_info: None,
            cnt_deleted_versions: None,
            cnt_comments: None,
            cnt_upload_shares: None,
            cnt_download_shares: None,
            recycle_bin_retention_period: None,
            has_activities_log: None,
            quota: None,
            is_favorite: None,
            branch_version: None,
            media_token: None,
            is_browsable: None,
            cnt_rooms: None,
            cnt_folders: None,
            cnt_files: None,
            auth_parent_id: None,
        }
    }

    fn folder_node(id: u64, name: &str) -> Node {
        Node {
            id,
            reference_id: None,
            node_type: NodeType::Folder,
            name: name.to_string(),
            timestamp_creation: None,
            timestamp_modification: None,
            parent_id: None,
            parent_path: Some("/room/".to_string()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            expire_at: None,
            hash: None,
            file_type: None,
            media_type: None,
            size: None,
            classification: None,
            notes: None,
            permissions: None,
            inherit_permissions: None,
            is_encrypted: Some(false),
            encryption_info: None,
            cnt_deleted_versions: None,
            cnt_comments: None,
            cnt_upload_shares: None,
            cnt_download_shares: None,
            recycle_bin_retention_period: None,
            has_activities_log: None,
            quota: None,
            is_favorite: None,
            branch_version: None,
            media_token: None,
            is_browsable: None,
            cnt_rooms: None,
            cnt_folders: None,
            cnt_files: None,
            auth_parent_id: None,
        }
    }

    #[test]
    fn test_effective_velocity_applies_defaults_and_clamps() {
        assert_eq!(effective_velocity(None), 1);
        assert_eq!(effective_velocity(Some(0)), 1);
        assert_eq!(effective_velocity(Some(255)), 10);
        assert_eq!(effective_velocity(Some(3)), 3);
    }

    #[test]
    fn test_concurrent_requests_for_velocity_scales_by_multiplier() {
        assert_eq!(concurrent_requests_for_velocity(None), 10);
        assert_eq!(concurrent_requests_for_velocity(Some(1)), 10);
        assert_eq!(concurrent_requests_for_velocity(Some(3)), 30);
        assert_eq!(concurrent_requests_for_velocity(Some(10)), 100);
        assert_eq!(concurrent_requests_for_velocity(Some(255)), 100);
    }

    #[tokio::test]
    async fn test_high_concurrency_bounded_execution_respects_cap() {
        let limit = concurrent_requests_for_velocity(Some(10));
        let peak = simulate_bounded_tasks(400, limit).await;
        assert!(peak <= limit);
        assert!(peak > 1);
    }

    #[tokio::test]
    async fn test_low_velocity_bounded_execution_respects_cap() {
        let limit = concurrent_requests_for_velocity(Some(1));
        let peak = simulate_bounded_tasks(80, limit).await;
        assert!(peak <= limit);
        assert!(peak > 1);
    }

    #[tokio::test]
    async fn test_maybe_build_share_message_uses_injected_creator_for_share_uploads() {
        let creator = MockShareLinkCreator::default();
        let node = file_node(1, Some(false));

        let message =
            maybe_build_share_message(&node, "file.txt", true, Some("pw".to_string()), &creator)
                .await
                .expect("share link message should be created");

        assert_eq!(
            message.as_deref(),
            Some("Shared file.txt.\n▶︎▶︎ https://example.com/public/download-shares/mock-link")
        );
        assert_eq!(creator.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_maybe_build_share_message_skips_creator_for_encrypted_nodes() {
        let creator = MockShareLinkCreator::default();
        let node = file_node(1, Some(true));

        let message =
            maybe_build_share_message(&node, "file.txt", true, Some("pw".to_string()), &creator)
                .await
                .expect("encrypted nodes should not be shared");

        assert!(message.is_none());
        assert_eq!(creator.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_upload_public_file_happy_path_with_http_mock() {
        let mut server = Server::new_async().await;

        let share_response = r#"{
            "isProtected": false,
            "createdAt": "2021-01-01T00:00:00.000Z",
            "isEncrypted": false
        }"#;
        let share_mock = server
            .mock("GET", "/api/v4/public/shares/uploads/test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(share_response)
            .create_async()
            .await;

        let system_info_response = r#"{
            "languageDefault": "en-US",
            "s3Hosts": [],
            "s3EnforceDirectUpload": false,
            "useS3Storage": false
        }"#;
        let system_info_mock = server
            .mock("GET", "/api/v4/public/system/info")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(system_info_response)
            .create_async()
            .await;

        let upload_channel_response = format!(
            r#"{{"uploadId":"upload-1","uploadUrl":"{}/mock/upload-target"}}"#,
            server.url()
        );
        let upload_channel_mock = server
            .mock("POST", "/api/v4/public/shares/uploads/test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(upload_channel_response)
            .create_async()
            .await;

        let upload_chunk_mock = server
            .mock("POST", "/mock/upload-target")
            .with_status(200)
            .create_async()
            .await;

        let finalize_response = r#"{
            "name": "source.txt",
            "size": 4,
            "createdAt": "2021-01-01T00:00:00.000Z"
        }"#;
        let finalize_mock = server
            .mock("PUT", "/api/v4/public/shares/uploads/test/upload-1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(finalize_response)
            .create_async()
            .await;

        let tmp_dir = unique_test_dir("dccmd-public-upload");
        tokio::fs::create_dir_all(&tmp_dir)
            .await
            .expect("create temp dir");
        let source = tmp_dir.join("source.txt");
        tokio::fs::write(&source, b"ABCD")
            .await
            .expect("write test source");

        upload_public_file(
            source.clone(),
            format!("{}/public/upload-shares/test", server.url()),
            &NoopProgressReporter,
        )
        .await
        .expect("public upload should succeed");

        share_mock.assert_async().await;
        system_info_mock.assert_async().await;
        upload_channel_mock.assert_async().await;
        upload_chunk_mock.assert_async().await;
        finalize_mock.assert_async().await;

        let _ = tokio::fs::remove_file(&source).await;
        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    }

    #[test]
    fn test_upload_error_reason_returns_original_message_for_invalid_path() {
        let reason = upload_error_reason(&DcCmdError::InvalidPath("missing".to_string()));
        assert_eq!(reason, "missing");
    }

    #[test]
    fn test_upload_outcome_failure_message_includes_preview_and_remainder() {
        let failures = (0..7)
            .map(|idx| UploadFailure {
                file: format!("/tmp/file-{idx}.txt"),
                reason: "Connection to DRACOON failed".to_string(),
            })
            .collect::<Vec<_>>();

        let outcome = UploadOutcome::from_failures("target", 10, 3, failures);
        let message = outcome.failure_message().expect("failure message");
        assert!(message.contains("Uploaded 3/10 file(s) to target; 7 failed."));
        assert!(message.contains("Failed /tmp/file-0.txt (Connection to DRACOON failed)"));
        assert!(message.contains("... and 2 more failed file(s)."));
    }

    #[tokio::test]
    async fn test_upload_files_aggregates_partial_failures_with_mock_api() {
        let api = MockUploadApi::with_results(vec![(
            "/tmp/b.txt".to_string(),
            Err(DcCmdError::InvalidArgument("network".to_string())),
        )]);
        let target = folder_node(1, "room");
        let parent_node = folder_node(11, "batch");
        let files = BTreeMap::from([
            (PathBuf::from("/tmp/a.txt"), (11, 10)),
            (PathBuf::from("/tmp/b.txt"), (11, 20)),
        ]);
        let parent_nodes = HashMap::from([(11, parent_node)]);

        let outcome = upload_files(
            &api,
            &target,
            files,
            parent_nodes,
            CmdUploadOptions::new(
                false,
                false,
                true,
                false,
                false,
                None,
                Some(1),
                None,
                None,
                None,
            ),
            &NoopProgressReporter,
        )
        .await
        .expect("upload outcome");

        let mut calls = api.calls();
        calls.sort();
        assert_eq!(
            calls,
            vec!["/tmp/a.txt".to_string(), "/tmp/b.txt".to_string()]
        );
        assert_eq!(outcome.requested_total, 2);
        assert_eq!(outcome.succeeded, 1);
        assert_eq!(outcome.failed, 1);
        assert!(outcome.partial);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(outcome.failures[0].file, "/tmp/b.txt");
        assert_eq!(outcome.failures[0].reason, "network");
    }

    #[tokio::test]
    async fn test_upload_files_aggregates_all_failed_with_mock_api() {
        let api = MockUploadApi::with_results(vec![
            (
                "/tmp/a.txt".to_string(),
                Err(DcCmdError::InvalidArgument("io".to_string())),
            ),
            (
                "/tmp/b.txt".to_string(),
                Err(DcCmdError::InvalidPath("missing".to_string())),
            ),
        ]);
        let target = folder_node(1, "room");
        let parent_node = folder_node(22, "batch");
        let files = BTreeMap::from([
            (PathBuf::from("/tmp/a.txt"), (22, 10)),
            (PathBuf::from("/tmp/b.txt"), (22, 20)),
        ]);
        let parent_nodes = HashMap::from([(22, parent_node)]);

        let outcome = upload_files(
            &api,
            &target,
            files,
            parent_nodes,
            CmdUploadOptions::new(
                false,
                false,
                true,
                false,
                false,
                None,
                Some(1),
                None,
                None,
                None,
            ),
            &NoopProgressReporter,
        )
        .await
        .expect("upload outcome");

        assert_eq!(outcome.requested_total, 2);
        assert_eq!(outcome.succeeded, 0);
        assert_eq!(outcome.failed, 2);
        assert!(!outcome.partial);
        assert_eq!(outcome.failures.len(), 2);
        assert!(outcome
            .failures
            .iter()
            .any(|failure| failure.file == "/tmp/a.txt" && failure.reason == "io"));
        assert!(outcome
            .failures
            .iter()
            .any(|failure| failure.file == "/tmp/b.txt" && failure.reason == "missing"));
    }
}
