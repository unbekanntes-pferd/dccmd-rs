use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use dco3::{
    auth::Disconnected,
    nodes::{Node, NodesSearchFilter, NodesSearchSortBy},
    Dracoon, Public, PublicDownload, SortOrder,
};
use futures_util::{stream, StreamExt};
use tracing::{debug, error, info, warn};

use crate::{
    app::nodes::{
        command::CmdDownloadOptions,
        download::api::DownloadApi,
        download::{
            DownloadFailure, DownloadJobState, DownloadOutcome, NodesDownloadService,
            TransferStateStore,
        },
        progress::{start_progress_bar, update_remaining_files_message},
    },
    core::{
        constants::{DEFAULT_CONCURRENT_MULTIPLIER, MAX_VELOCITY, MIN_VELOCITY},
        models::DcCmdError,
    },
};

#[derive(Clone)]
struct DownloadTask {
    node: Node,
    target: String,
}

impl<F, S> NodesDownloadService<F, S>
where
    F: crate::app::nodes::filesystem::Filesystem,
    S: TransferStateStore,
{
    pub(super) async fn get_files<A: DownloadApi>(
        &self,
        api: &A,
        parent_node: &Node,
    ) -> Result<Vec<Node>, DcCmdError> {
        let files = self
            .search_nodes_all_pages_with_filter_sort(
                api,
                "*",
                Some(parent_node.id),
                Some(-1),
                NodesSearchFilter::is_file(),
                NodesSearchSortBy::size(SortOrder::Desc),
            )
            .await?;

        let actual_count = files.items.len() as u64;
        if files.range.total != actual_count {
            warn!(
                "Total file count mismatch - expected: {}, actual: {}, difference: {} (check error logs)",
                files.range.total,
                actual_count,
                files.range.total - actual_count
            );
        }

        Ok(files.get_files())
    }

    pub(super) async fn download_public_file(
        &self,
        dracoon: &Dracoon<Disconnected>,
        source: String,
        target: String,
        download_opts: CmdDownloadOptions,
    ) -> Result<DownloadOutcome, DcCmdError> {
        if download_opts.recursive {
            return Err(DcCmdError::InvalidArgument(
                "Recursive download not supported for public download shares".to_string(),
            ));
        }

        let access_key = Self::parse_public_download_access_key(&source)?;
        let public_download_share = dracoon
            .public()
            .get_public_download_share(access_key)
            .await?;
        let file_name = public_download_share.file_name.clone();

        let resolved_target =
            self.resolve_single_file_target(&target, &public_download_share.file_name);
        let mut out_file = tokio::fs::File::create(&resolved_target)
            .await
            .or(Err(DcCmdError::IoError))?;

        let progress_bar = start_progress_bar(
            self.progress(),
            public_download_share.size,
            Some(&file_name),
        );
        let progress_bar_mv = progress_bar.clone();

        dracoon
            .public()
            .download(
                access_key,
                public_download_share.clone(),
                download_opts.share_password,
                &mut out_file,
                Some(Box::new(move |progress, _| {
                    progress_bar_mv.inc(progress);
                })),
                None,
            )
            .await?;

        progress_bar.finish_with_message(&format!("{file_name} complete"));
        info!("Download of public file {file_name} complete.");

        Ok(DownloadOutcome::success(resolved_target, 1))
    }

    pub(super) async fn download_single_file<A: DownloadApi>(
        &self,
        api: &A,
        node: &Node,
        target: &str,
    ) -> Result<DownloadOutcome, DcCmdError> {
        info!("Attempting download of node {}.", node.name);
        info!("Target: {}", target);

        let resolved_target = self.resolve_single_file_target(target, &node.name);
        let mut out_file = tokio::fs::File::create(&resolved_target)
            .await
            .or(Err(DcCmdError::IoError))?;

        let node_name = node.name.clone();
        let progress_bar =
            start_progress_bar(self.progress(), node.size.unwrap_or(0), Some(&node_name));

        let progress_bar_mv = progress_bar.clone();
        api.download_node(
            node,
            &mut out_file,
            Some(Box::new(move |progress, _| {
                progress_bar_mv.inc(progress);
            })),
        )
        .await?;

        progress_bar.finish_with_message(&format!("{node_name} complete"));
        info!("Download of node {node_name} complete.");

        Ok(DownloadOutcome::success(resolved_target, 1))
    }

    pub(super) async fn download_files_with_state<
        A: DownloadApi + Clone + Send + Sync + 'static,
    >(
        &self,
        api: &A,
        files: Vec<Node>,
        file_targets: HashMap<u64, String>,
        velocity: Option<u8>,
        target_display: &str,
        job_state: &DownloadJobState<'_>,
    ) -> Result<DownloadOutcome, DcCmdError> {
        info!("Attempting download of {} files.", files.len());
        info!("Target: {}", target_display);

        let velocity = Self::effective_velocity(velocity);
        let concurrent_reqs = usize::from(velocity) * DEFAULT_CONCURRENT_MULTIPLIER as usize;

        let mut tasks = Vec::new();
        for file in files
            .into_iter()
            .filter(|file| !job_state.completed_file_ids.contains(&file.id))
        {
            let target = file_targets.get(&file.id).cloned().ok_or_else(|| {
                DcCmdError::InvalidPath(format!("Target not found for node {}", file.id))
            })?;
            tasks.push(DownloadTask { node: file, target });
        }

        let total_size = tasks.iter().map(|task| task.node.size.unwrap_or(0)).sum();
        let file_count = tasks.len();
        let message = format!("Downloading {file_count} files");
        let progress_bar = start_progress_bar(self.progress(), total_size, Some(&message));

        let remaining_files = Arc::new(AtomicU64::new(file_count as u64));
        let task_results = stream::iter(tasks.into_iter().map(|task| {
            let api_client = api.clone();
            let progress_bar_mv = progress_bar.clone();
            let progress_bar_inc = progress_bar.clone();
            let remaining_files = remaining_files.clone();

            async move {
                let node_id = task.node.id;
                let node_name = task.node.name.clone();
                let target = task.target;

                let result = async {
                    let mut out_file = tokio::fs::File::create(&target)
                        .await
                        .or(Err(DcCmdError::IoError))?;
                    api_client
                        .download_node(
                            &task.node,
                            &mut out_file,
                            Some(Box::new(move |progress, _| {
                                progress_bar_mv.inc(progress);
                            })),
                        )
                        .await
                }
                .await;

                let remaining = remaining_files
                    .fetch_sub(1, Ordering::Relaxed)
                    .saturating_sub(1);
                update_remaining_files_message(progress_bar_inc.as_ref(), "Downloading", remaining);

                (node_id, node_name, target, result)
            }
        }))
        .buffer_unordered(concurrent_reqs)
        .collect::<Vec<_>>()
        .await;

        let mut succeeded = 0_u64;
        let mut failures = Vec::new();
        for (node_id, node_name, target, result) in task_results {
            match result {
                Ok(()) => {
                    if let Err(err) = self
                        .state_store()
                        .mark_complete(job_state.job_id, node_id)
                        .await
                    {
                        failures.push(DownloadFailure {
                            node_id: Some(node_id),
                            node_name,
                            target,
                            reason: format!("checkpoint error: {err}"),
                        });
                    } else {
                        succeeded += 1;
                        debug!("Downloaded node {node_id} to {target}");
                    }
                }
                Err(e) => {
                    error!("Error downloading file {node_name} ({node_id}): {e}");
                    failures.push(DownloadFailure {
                        node_id: Some(node_id),
                        node_name,
                        target,
                        reason: Self::download_error_reason(&e),
                    });
                }
            }
        }

        progress_bar.finish_with_message(&format!(
            "Download to {target_display} complete ({succeeded}/{file_count})"
        ));

        let requested_total = file_count as u64;
        let outcome =
            DownloadOutcome::from_failures(target_display, requested_total, succeeded, failures);
        info!(
            "Download completed. Requested: {}, succeeded: {}, failed: {}.",
            outcome.requested_total, outcome.succeeded, outcome.failed
        );

        Ok(outcome)
    }

    fn download_error_reason(error: &DcCmdError) -> String {
        match error {
            DcCmdError::InvalidArgument(msg)
            | DcCmdError::InvalidPath(msg)
            | DcCmdError::InvalidUrl(msg) => msg.clone(),
            _ => error.to_string(),
        }
    }

    fn parse_public_download_access_key(source: &str) -> Result<&str, DcCmdError> {
        let access_key = source
            .split('/')
            .next_back()
            .ok_or_else(|| DcCmdError::InvalidPath(source.to_string()))?;

        if access_key.is_empty() {
            return Err(DcCmdError::InvalidPath(source.to_string()));
        }

        Ok(access_key)
    }

    fn effective_velocity(velocity: Option<u8>) -> u8 {
        velocity
            .unwrap_or(MIN_VELOCITY)
            .clamp(MIN_VELOCITY, MAX_VELOCITY)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        path::Path,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use async_trait::async_trait;
    use dco3::{
        models::{Range, RangedItems},
        nodes::{models::NodeType, Node, NodeList},
        ListAllParams,
    };
    use mockito::Server;
    use tokio::io::{AsyncWrite, AsyncWriteExt};

    use crate::{
        app::{
            auth::AuthService,
            nodes::{
                api::NodesApi,
                command::CmdDownloadOptions,
                download::api::DownloadApi,
                download::{DownloadJobState, NodesDownloadService, NoopTransferStateStore},
                filesystem::OSFileSystem,
            },
        },
        core::models::DcCmdError,
    };

    #[derive(Clone)]
    struct MockDownloadApi {
        search_responses: Arc<Mutex<Vec<NodeList>>>,
        search_calls: Arc<Mutex<u32>>,
        fail_download_for_ids: Arc<HashSet<u64>>,
        downloaded_ids: Arc<Mutex<Vec<u64>>>,
        download_delay: Option<Duration>,
        in_flight_downloads: Arc<AtomicUsize>,
        peak_in_flight_downloads: Arc<AtomicUsize>,
    }

    impl MockDownloadApi {
        fn new(search_responses: Vec<NodeList>) -> Self {
            Self {
                search_responses: Arc::new(Mutex::new(search_responses)),
                search_calls: Arc::new(Mutex::new(0)),
                fail_download_for_ids: Arc::new(HashSet::new()),
                downloaded_ids: Arc::new(Mutex::new(Vec::new())),
                download_delay: None,
                in_flight_downloads: Arc::new(AtomicUsize::new(0)),
                peak_in_flight_downloads: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn with_failed_download_ids(mut self, ids: &[u64]) -> Self {
            self.fail_download_for_ids = Arc::new(ids.iter().copied().collect());
            self
        }

        fn with_download_delay(mut self, delay: Duration) -> Self {
            self.download_delay = Some(delay);
            self
        }

        fn peak_in_flight_downloads(&self) -> usize {
            self.peak_in_flight_downloads.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl NodesApi for MockDownloadApi {
        async fn get_node(&self, _node_id: u64) -> Result<Node, DcCmdError> {
            Err(DcCmdError::InvalidArgument(
                "node lookup not configured".to_string(),
            ))
        }

        async fn get_node_from_path(&self, _node_path: &str) -> Result<Option<Node>, DcCmdError> {
            Ok(None)
        }

        async fn get_nodes(
            &self,
            _parent_id: Option<u64>,
            _managed: Option<bool>,
            _params: Option<ListAllParams>,
        ) -> Result<NodeList, DcCmdError> {
            Ok(empty_node_list())
        }

        async fn search_nodes(
            &self,
            _search_string: &str,
            _parent_id: Option<u64>,
            _depth_level: Option<i8>,
            _params: Option<ListAllParams>,
        ) -> Result<NodeList, DcCmdError> {
            let mut calls = self.search_calls.lock().expect("lock poisoned");
            *calls += 1;
            drop(calls);

            let mut responses = self.search_responses.lock().expect("lock poisoned");
            if responses.len() > 1 {
                Ok(responses.remove(0))
            } else {
                Ok(responses.first().cloned().unwrap_or_else(empty_node_list))
            }
        }

        async fn delete_node(&self, _node_id: u64) -> Result<(), DcCmdError> {
            Ok(())
        }

        async fn delete_nodes(&self, _node_ids: Vec<u64>) -> Result<(), DcCmdError> {
            Ok(())
        }

        async fn copy_nodes(
            &self,
            _node_ids: Vec<u64>,
            _target_parent_id: u64,
        ) -> Result<(), DcCmdError> {
            Ok(())
        }

        async fn create_folder(
            &self,
            _node_name: &str,
            _parent_id: u64,
            _classification: Option<u8>,
            _notes: Option<String>,
        ) -> Result<(), DcCmdError> {
            Ok(())
        }

        async fn create_room(
            &self,
            _node_name: &str,
            _parent_id: u64,
            _classification: u8,
            _inherit_permissions: bool,
            _admin_ids: Option<Vec<u64>>,
        ) -> Result<(), DcCmdError> {
            Ok(())
        }
    }

    #[async_trait]
    impl DownloadApi for MockDownloadApi {
        async fn download_node<'w>(
            &'w self,
            node: &Node,
            writer: &'w mut (dyn AsyncWrite + Send + Unpin),
            _callback: Option<dco3::nodes::models::DownloadProgressCallback>,
        ) -> Result<(), DcCmdError> {
            let now = self.in_flight_downloads.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak_in_flight_downloads
                .fetch_max(now, Ordering::SeqCst);

            if let Some(delay) = self.download_delay {
                tokio::time::sleep(delay).await;
            }

            let result = if self.fail_download_for_ids.contains(&node.id) {
                Err(DcCmdError::InvalidArgument(format!(
                    "download failed for node {}",
                    node.id
                )))
            } else {
                self.downloaded_ids
                    .lock()
                    .expect("lock poisoned")
                    .push(node.id);
                writer
                    .write_all(format!("payload-{}", node.id).as_bytes())
                    .await
                    .map_err(|_| DcCmdError::IoError)
            };

            self.in_flight_downloads.fetch_sub(1, Ordering::SeqCst);
            result
        }
    }

    fn node(id: u64, name: &str, node_type: NodeType, parent_path: &str) -> Node {
        Node {
            id,
            reference_id: None,
            node_type,
            name: name.to_string(),
            timestamp_creation: None,
            timestamp_modification: None,
            parent_id: None,
            parent_path: Some(parent_path.to_string()),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            expire_at: None,
            hash: None,
            file_type: None,
            media_type: None,
            size: Some(10),
            classification: None,
            notes: None,
            permissions: None,
            inherit_permissions: None,
            is_encrypted: None,
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

    fn node_list(items: Vec<Node>, total: u64) -> NodeList {
        RangedItems {
            range: Range {
                offset: 0,
                limit: 500,
                total,
            },
            items,
        }
    }

    fn empty_node_list() -> NodeList {
        node_list(Vec::new(), 0)
    }

    fn unique_test_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }

    #[tokio::test]
    async fn test_get_files_filters_non_file_nodes() {
        let api = MockDownloadApi::new(vec![node_list(
            vec![
                node(11, "a.txt", NodeType::File, "/room/root/"),
                node(22, "folder-a", NodeType::Folder, "/room/root/"),
            ],
            2,
        )]);
        let parent = node(1, "root", NodeType::Room, "/room/");
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let files = service.get_files(&api, &parent).await.unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, 11);
    }

    #[tokio::test]
    async fn test_get_files_fetches_next_page() {
        let api = MockDownloadApi::new(vec![
            node_list(vec![node(11, "a.txt", NodeType::File, "/room/root/")], 700),
            node_list(vec![node(12, "b.txt", NodeType::File, "/room/root/")], 700),
        ]);
        let parent = node(1, "root", NodeType::Room, "/room/");
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let files = service.get_files(&api, &parent).await.unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(*api.search_calls.lock().expect("lock poisoned"), 2);
    }

    #[tokio::test]
    async fn test_download_files_errors_when_target_missing() {
        let api = MockDownloadApi::new(vec![empty_node_list()]);
        let files = vec![node(11, "a.txt", NodeType::File, "/room/root/")];
        let targets = HashMap::new();
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let result = service
            .download_files_with_state(
                &api,
                files,
                targets,
                Some(1),
                "/virtual",
                &DownloadJobState {
                    job_id: "noop",
                    completed_file_ids: &HashSet::new(),
                },
            )
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidPath(msg)) if msg == "Target not found for node 11"
        ));
    }

    #[tokio::test]
    async fn test_download_files_success_writes_all_targets() {
        let api = MockDownloadApi::new(vec![empty_node_list()]);
        let tmp_dir = unique_test_dir("dccmd-download-success");
        tokio::fs::create_dir_all(&tmp_dir)
            .await
            .expect("create dir");

        let files = vec![
            node(11, "a.txt", NodeType::File, "/room/root/"),
            node(12, "b.txt", NodeType::File, "/room/root/"),
        ];
        let target_a = tmp_dir.join("a.txt");
        let target_b = tmp_dir.join("b.txt");
        let targets = HashMap::from([
            (11_u64, target_a.to_string_lossy().to_string()),
            (12_u64, target_b.to_string_lossy().to_string()),
        ]);
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let outcome = service
            .download_files_with_state(
                &api,
                files,
                targets,
                Some(1),
                &tmp_dir.to_string_lossy(),
                &DownloadJobState {
                    job_id: "noop",
                    completed_file_ids: &HashSet::new(),
                },
            )
            .await
            .expect("download should succeed");

        assert_eq!(outcome.requested_total, 2);
        assert_eq!(outcome.succeeded, 2);
        assert_eq!(outcome.failed, 0);
        assert!(!outcome.partial);
        assert!(Path::new(&target_a).exists());
        assert!(Path::new(&target_b).exists());
        assert_eq!(
            api.downloaded_ids
                .lock()
                .expect("lock poisoned")
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            HashSet::from([11_u64, 12_u64])
        );

        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    }

    #[tokio::test]
    async fn test_download_files_aggregates_partial_download_error() {
        let api = MockDownloadApi::new(vec![empty_node_list()]).with_failed_download_ids(&[12]);
        let tmp_dir = unique_test_dir("dccmd-download-partial");
        tokio::fs::create_dir_all(&tmp_dir)
            .await
            .expect("create dir");

        let files = vec![
            node(11, "a.txt", NodeType::File, "/room/root/"),
            node(12, "b.txt", NodeType::File, "/room/root/"),
        ];
        let target_a = tmp_dir.join("fail-a.txt");
        let target_b = tmp_dir.join("fail-b.txt");
        let targets = HashMap::from([
            (11_u64, target_a.to_string_lossy().to_string()),
            (12_u64, target_b.to_string_lossy().to_string()),
        ]);
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let outcome = service
            .download_files_with_state(
                &api,
                files,
                targets,
                Some(1),
                &tmp_dir.to_string_lossy(),
                &DownloadJobState {
                    job_id: "noop",
                    completed_file_ids: &HashSet::new(),
                },
            )
            .await
            .expect("partial failures should be aggregated");

        assert_eq!(outcome.requested_total, 2);
        assert_eq!(outcome.succeeded, 1);
        assert_eq!(outcome.failed, 1);
        assert!(outcome.partial);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(outcome.failures[0].node_id, Some(12));
        assert_eq!(outcome.failures[0].node_name, "b.txt");
        assert_eq!(outcome.failures[0].reason, "download failed for node 12");
        assert!(Path::new(&target_a).exists());
        assert!(Path::new(&target_b).exists());

        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    }

    #[tokio::test]
    async fn test_download_files_aggregates_all_failed_downloads() {
        let api = MockDownloadApi::new(vec![empty_node_list()]).with_failed_download_ids(&[11, 12]);
        let tmp_dir = unique_test_dir("dccmd-download-all-fail");
        tokio::fs::create_dir_all(&tmp_dir)
            .await
            .expect("create dir");

        let files = vec![
            node(11, "a.txt", NodeType::File, "/room/root/"),
            node(12, "b.txt", NodeType::File, "/room/root/"),
        ];
        let target_a = tmp_dir.join("all-fail-a.txt");
        let target_b = tmp_dir.join("all-fail-b.txt");
        let targets = HashMap::from([
            (11_u64, target_a.to_string_lossy().to_string()),
            (12_u64, target_b.to_string_lossy().to_string()),
        ]);
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let outcome = service
            .download_files_with_state(
                &api,
                files,
                targets,
                Some(1),
                &tmp_dir.to_string_lossy(),
                &DownloadJobState {
                    job_id: "noop",
                    completed_file_ids: &HashSet::new(),
                },
            )
            .await
            .expect("all failures should still return aggregated outcome");

        assert_eq!(outcome.requested_total, 2);
        assert_eq!(outcome.succeeded, 0);
        assert_eq!(outcome.failed, 2);
        assert!(!outcome.partial);
        assert_eq!(outcome.failures.len(), 2);
        assert!(Path::new(&target_a).exists());
        assert!(Path::new(&target_b).exists());

        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    }

    #[tokio::test]
    async fn test_download_files_high_concurrency_respects_velocity_cap() {
        let api = MockDownloadApi::new(vec![empty_node_list()])
            .with_download_delay(Duration::from_millis(5));
        let tmp_dir = unique_test_dir("dccmd-download-high-conc");
        tokio::fs::create_dir_all(&tmp_dir)
            .await
            .expect("create dir");

        let files = (1_u64..=160)
            .map(|id| node(id, &format!("n{id}.txt"), NodeType::File, "/room/root/"))
            .collect::<Vec<_>>();
        let targets = (1_u64..=160)
            .map(|id| {
                (
                    id,
                    tmp_dir
                        .join(format!("n{id}.txt"))
                        .to_string_lossy()
                        .to_string(),
                )
            })
            .collect::<HashMap<_, _>>();
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let outcome = service
            .download_files_with_state(
                &api,
                files,
                targets,
                Some(10),
                &tmp_dir.to_string_lossy(),
                &DownloadJobState {
                    job_id: "noop",
                    completed_file_ids: &HashSet::new(),
                },
            )
            .await
            .expect("concurrent downloads should complete");

        assert_eq!(outcome.requested_total, 160);
        assert_eq!(outcome.succeeded, 160);
        assert_eq!(outcome.failed, 0);
        assert!(api.peak_in_flight_downloads() <= 100);
        assert!(api.peak_in_flight_downloads() > 1);

        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    }

    #[tokio::test]
    async fn test_download_files_low_velocity_limits_parallelism() {
        let api = MockDownloadApi::new(vec![empty_node_list()])
            .with_download_delay(Duration::from_millis(5));
        let tmp_dir = unique_test_dir("dccmd-download-low-conc");
        tokio::fs::create_dir_all(&tmp_dir)
            .await
            .expect("create dir");

        let files = (1_u64..=80)
            .map(|id| node(id, &format!("n{id}.txt"), NodeType::File, "/room/root/"))
            .collect::<Vec<_>>();
        let targets = (1_u64..=80)
            .map(|id| {
                (
                    id,
                    tmp_dir
                        .join(format!("low-n{id}.txt"))
                        .to_string_lossy()
                        .to_string(),
                )
            })
            .collect::<HashMap<_, _>>();
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let outcome = service
            .download_files_with_state(
                &api,
                files,
                targets,
                Some(1),
                &tmp_dir.to_string_lossy(),
                &DownloadJobState {
                    job_id: "noop",
                    completed_file_ids: &HashSet::new(),
                },
            )
            .await
            .expect("downloads should complete at low velocity");

        assert_eq!(outcome.requested_total, 80);
        assert_eq!(outcome.succeeded, 80);
        assert_eq!(outcome.failed, 0);
        assert!(api.peak_in_flight_downloads() <= 10);
        assert!(api.peak_in_flight_downloads() > 1);

        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    }

    #[tokio::test]
    async fn test_download_public_file_rejects_recursive() {
        let service = NodesDownloadService::new();
        let dracoon = AuthService::new()
            .init_public("https://example.com/public/download-shares/abc123")
            .await
            .expect("public client");

        let result = service
            .download_public_file(
                &dracoon,
                "https://example.com/public/download-shares/abc123".to_string(),
                "/virtual/out.txt".to_string(),
                CmdDownloadOptions::new(true, None, None, false),
            )
            .await;

        assert!(matches!(
            result,
            Err(DcCmdError::InvalidArgument(msg))
            if msg == "Recursive download not supported for public download shares"
        ));
    }

    #[test]
    fn test_parse_public_download_access_key() {
        let access_key = NodesDownloadService::<OSFileSystem, NoopTransferStateStore>::parse_public_download_access_key(
            "https://example.com/public/download-shares/abc123",
        )
        .expect("access key");

        assert_eq!(access_key, "abc123");
    }

    #[test]
    fn test_parse_public_download_access_key_rejects_empty_tail() {
        let err = NodesDownloadService::<OSFileSystem, NoopTransferStateStore>::parse_public_download_access_key(
            "https://example.com/public/download-shares/",
        )
        .expect_err("empty tail should fail");

        assert!(matches!(err, DcCmdError::InvalidPath(_)));
    }

    #[tokio::test]
    async fn test_download_public_file_happy_path_with_http_mock() {
        let mut server = Server::new_async().await;

        let share_response = r#"{
            "isProtected": false,
            "fileName": "public-file.txt",
            "size": 4,
            "limitReached": false,
            "creatorName": "string",
            "createdAt": "2021-01-01T00:00:00.000Z",
            "hasDownloadLimit": false,
            "mediaType": "text/plain",
            "name": "public-file.txt",
            "creatorUsername": "string",
            "expireAt": "2027-01-01T00:00:00.000Z",
            "notes": "string",
            "isEncrypted": false,
            "virusProtectionInfo": {
                "verdict": "NOT_SCANNING",
                "lastCheckedAt": "2021-01-01T00:00:00.000Z",
                "sha256": "string"
            }
        }"#;
        let share_mock = server
            .mock("GET", "/api/v4/public/shares/downloads/test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(share_response)
            .create_async()
            .await;

        let download_url_response = format!(
            r#"{{"downloadUrl":"{}/mock/download-target"}}"#,
            server.url()
        );
        let token_mock = server
            .mock("POST", "/api/v4/public/shares/downloads/test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(download_url_response)
            .create_async()
            .await;

        let download_mock = server
            .mock("GET", "/mock/download-target")
            .with_status(200)
            .with_header("content-type", "application/octet-stream")
            .with_body("ABCD")
            .create_async()
            .await;

        let tmp_dir = unique_test_dir("dccmd-public-download");
        tokio::fs::create_dir_all(&tmp_dir)
            .await
            .expect("create dir");

        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);
        let public_client = AuthService::new()
            .init_public(&format!("{}/public/download-shares/test", server.url()))
            .await
            .expect("public client");
        let outcome = service
            .download_public_file(
                &public_client,
                format!("{}/public/download-shares/test", server.url()),
                tmp_dir.to_string_lossy().to_string(),
                CmdDownloadOptions::new(false, None, None, false),
            )
            .await
            .expect("public download should succeed");

        share_mock.assert_async().await;
        token_mock.assert_async().await;
        download_mock.assert_async().await;

        assert_eq!(outcome.succeeded, 1);
        assert_eq!(outcome.failed, 0);
        assert!(Path::new(&outcome.target_root).exists());

        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
    }
}
