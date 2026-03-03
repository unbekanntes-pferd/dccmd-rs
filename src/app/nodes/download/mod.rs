pub mod api;
mod containers;
mod files;

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use dco3::{
    nodes::{models::NodeType, Node, NodeList, NodesSearchFilter, NodesSearchSortBy},
    ListAllParams,
};
use tracing::{debug, error, info};

use crate::{
    app::{
        auth::AuthService,
        nodes::{api::NodesApi, command::CmdDownloadOptions},
    },
    core::{models::DcCmdError, utils::strings::parse_path},
};

use super::{
    common::{is_search_query, NodesPathPaginationHelper},
    filesystem::{Filesystem, OSFileSystem},
    progress::{NoopProgressReporter, ProgressReporter},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteNodeRef {
    pub id: u64,
    pub name: String,
    pub parent_path: Option<String>,
}

impl From<&Node> for RemoteNodeRef {
    fn from(value: &Node) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            parent_path: value.parent_path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadFailure {
    pub node_id: Option<u64>,
    pub node_name: String,
    pub target: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOutcome {
    pub requested_total: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub failures: Vec<DownloadFailure>,
    pub target_root: String,
    pub partial: bool,
}

impl DownloadOutcome {
    pub fn success(target_root: impl Into<String>, count: u64) -> Self {
        Self {
            requested_total: count,
            succeeded: count,
            failed: 0,
            failures: Vec::new(),
            target_root: target_root.into(),
            partial: false,
        }
    }

    pub fn from_failures(
        target_root: impl Into<String>,
        requested_total: u64,
        succeeded: u64,
        failures: Vec<DownloadFailure>,
    ) -> Self {
        let failed = failures.len() as u64;
        Self {
            requested_total,
            succeeded,
            failed,
            failures,
            target_root: target_root.into(),
            partial: failed > 0 && succeeded > 0,
        }
    }
}

#[derive(Debug)]
pub struct DownloadContainerPlan {
    pub root_target: String,
    pub directories: Vec<String>,
    pub file_targets: HashMap<u64, String>,
}

const SEARCH_PAGE_SIZE: u64 = 500;

pub(super) struct DownloadJobState<'a> {
    pub job_id: &'a str,
    pub completed_file_ids: &'a HashSet<u64>,
}

#[async_trait]
pub trait TransferStateStore: Send + Sync {
    async fn get_or_create_job(&self, source: &str, target: &str) -> Result<String, DcCmdError>;

    async fn get_completed_file_ids(&self, job_id: &str) -> Result<HashSet<u64>, DcCmdError>;

    async fn mark_complete(&self, job_id: &str, node_id: u64) -> Result<(), DcCmdError>;

    async fn complete_job(&self, job_id: &str) -> Result<(), DcCmdError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopTransferStateStore;

#[async_trait]
impl TransferStateStore for NoopTransferStateStore {
    async fn get_or_create_job(&self, source: &str, target: &str) -> Result<String, DcCmdError> {
        Ok(format!("noop:{source}->{target}"))
    }

    async fn get_completed_file_ids(&self, _job_id: &str) -> Result<HashSet<u64>, DcCmdError> {
        Ok(HashSet::new())
    }

    async fn mark_complete(&self, _job_id: &str, _node_id: u64) -> Result<(), DcCmdError> {
        Ok(())
    }

    async fn complete_job(&self, _job_id: &str) -> Result<(), DcCmdError> {
        Ok(())
    }
}

pub struct NodesDownloadService<
    F: Filesystem = OSFileSystem,
    S: TransferStateStore = NoopTransferStateStore,
> {
    fs: F,
    state_store: S,
    path_pagination: NodesPathPaginationHelper,
    progress: Arc<dyn ProgressReporter>,
}

impl NodesDownloadService<OSFileSystem, NoopTransferStateStore> {
    pub fn new() -> Self {
        Self::with_dependencies(OSFileSystem, NoopTransferStateStore)
    }
}

impl<F: Filesystem, S: TransferStateStore> NodesDownloadService<F, S> {
    pub fn with_dependencies(fs: F, state_store: S) -> Self {
        Self::with_progress(fs, state_store, Arc::new(NoopProgressReporter))
    }

    pub fn with_progress(fs: F, state_store: S, progress: Arc<dyn ProgressReporter>) -> Self {
        Self {
            fs,
            state_store,
            path_pagination: NodesPathPaginationHelper::new(),
            progress,
        }
    }

    pub fn resolve_single_file_target(&self, target: &str, file_name: &str) -> String {
        if self.filesystem().is_dir(target) {
            self.filesystem().join(target, file_name)
        } else {
            self.filesystem().normalize(target)
        }
    }

    pub fn resolve_targets_from_base(
        &self,
        target_base: &str,
        files: &[RemoteNodeRef],
    ) -> HashMap<u64, String> {
        files
            .iter()
            .map(|file| (file.id, self.filesystem().join(target_base, &file.name)))
            .collect()
    }

    pub fn plan_container_download(
        &self,
        target_base: &str,
        container: &RemoteNodeRef,
        folders: &[RemoteNodeRef],
        files: &[RemoteNodeRef],
    ) -> Result<DownloadContainerPlan, DcCmdError> {
        let root_target = self.filesystem().join(target_base, &container.name);
        let mut directories = BTreeSet::new();
        directories.insert(self.filesystem().normalize(&root_target));

        for folder in folders {
            let relative = self.relative_path_segments(container, folder)?;
            if relative.is_empty() {
                continue;
            }
            let resolved = self.join_segments(&root_target, &relative);
            directories.insert(self.filesystem().normalize(&resolved));
        }

        let mut file_targets = HashMap::new();
        for file in files {
            let relative = self.relative_path_segments(container, file)?;
            if relative.is_empty() {
                return Err(DcCmdError::InvalidPath(format!(
                    "Invalid relative file target for node {}",
                    file.name
                )));
            }
            let resolved = self.join_segments(&root_target, &relative);
            file_targets.insert(file.id, self.filesystem().normalize(&resolved));
        }

        Ok(DownloadContainerPlan {
            root_target: self.filesystem().normalize(&root_target),
            directories: directories.into_iter().collect(),
            file_targets,
        })
    }

    pub fn create_directories(&self, directories: &[String]) -> Result<(), DcCmdError> {
        for dir in directories {
            self.filesystem().create_dir_all(dir)?;
        }
        Ok(())
    }

    fn filesystem(&self) -> &F {
        &self.fs
    }

    pub fn state_store(&self) -> &S {
        &self.state_store
    }

    pub(super) fn progress(&self) -> &dyn ProgressReporter {
        self.progress.as_ref()
    }

    fn join_segments(&self, base: &str, segments: &[String]) -> String {
        segments.iter().fold(base.to_string(), |acc, segment| {
            self.filesystem().join(&acc, segment)
        })
    }

    fn relative_path_segments(
        &self,
        container: &RemoteNodeRef,
        child: &RemoteNodeRef,
    ) -> Result<Vec<String>, DcCmdError> {
        let container_path = self.full_remote_segments(container)?;
        let child_path = self.full_remote_segments(child)?;

        if child_path.len() < container_path.len()
            || child_path[..container_path.len()] != container_path
        {
            return Err(DcCmdError::InvalidPath(format!(
                "Node '{}' is outside container '{}'",
                child.name, container.name
            )));
        }

        Ok(child_path[container_path.len()..].to_vec())
    }

    fn full_remote_segments(&self, node: &RemoteNodeRef) -> Result<Vec<String>, DcCmdError> {
        let parent_path = node.parent_path.as_deref().ok_or_else(|| {
            DcCmdError::InvalidPath(format!("Node '{}' has no parent path", node.name))
        })?;

        let mut segments = split_remote_path(parent_path)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        segments.push(node.name.clone());
        Ok(segments)
    }

    pub(super) async fn search_nodes_all_pages_with_filter_sort<A: NodesApi>(
        &self,
        api: &A,
        search_string: &str,
        parent_id: Option<u64>,
        depth_level: Option<i8>,
        filter: NodesSearchFilter,
        sort: NodesSearchSortBy,
    ) -> Result<NodeList, DcCmdError> {
        let params = ListAllParams::builder()
            .with_filter(filter.clone())
            .with_sort(sort.clone())
            .build();

        let mut nodes = api
            .search_nodes(search_string, parent_id, depth_level, Some(params))
            .await?;

        if nodes.range.total > SEARCH_PAGE_SIZE {
            let mut offset = SEARCH_PAGE_SIZE;
            while offset <= nodes.range.total {
                let params = ListAllParams::builder()
                    .with_filter(filter.clone())
                    .with_sort(sort.clone())
                    .with_offset(offset)
                    .build();
                let next_page = api
                    .search_nodes(search_string, parent_id, depth_level, Some(params))
                    .await?;
                nodes.items.extend(next_page.items);
                offset += SEARCH_PAGE_SIZE;
            }
        }

        Ok(nodes)
    }

    pub async fn download(
        &self,
        source: String,
        target: String,
        download_opts: CmdDownloadOptions,
    ) -> Result<DownloadOutcome, DcCmdError> {
        debug!("Downloading {} to {}", source, target);
        debug!("Velocity: {}", download_opts.velocity.unwrap_or(1));

        if source.contains("/public/download-shares/") {
            return self
                .download_public_file(source, target, download_opts)
                .await;
        }

        let auth_service = AuthService::new();
        let mut session = auth_service
            .connect(&source, download_opts.auth, true)
            .await?;

        let (parent_path, node_name, _) = parse_path(&source, session.base_url())
            .or(Err(DcCmdError::InvalidPath(source.clone())))?;
        let node_path = format!("{parent_path}{node_name}/");

        let lookup_api = session.client().clone();
        let node = if is_search_query(&node_name) {
            debug!("Searching for query {}", node_name);
            debug!("Parent path {}", parent_path);
            lookup_api.get_node_from_path(&parent_path).await?
        } else {
            lookup_api.get_node_from_path(&node_path).await?
        };

        let Some(node) = node else {
            error!("Node not found");
            return Err(DcCmdError::InvalidPath(source));
        };

        if node.is_encrypted == Some(true) {
            session = auth_service
                .ensure_encryption(session, download_opts.encryption_password)
                .await?;
        }

        let api_client = session.into_client();

        if is_search_query(&node_name) {
            info!("Attempting download of search query {}.", node_name);
            let files = self
                .search_query_files(&api_client, &node_name, &parent_path)
                .await?;
            info!("Found {} files.", files.len());

            let file_refs = files.iter().map(RemoteNodeRef::from).collect::<Vec<_>>();
            let targets = self.resolve_targets_from_base(&target, &file_refs);
            let job_id = self.state_store.get_or_create_job(&source, &target).await?;
            let completed_file_ids = self.state_store.get_completed_file_ids(&job_id).await?;
            let job_state = DownloadJobState {
                job_id: &job_id,
                completed_file_ids: &completed_file_ids,
            };

            let outcome = self
                .download_files_with_state(
                    &api_client,
                    files,
                    targets,
                    download_opts.velocity,
                    &target,
                    &job_state,
                )
                .await?;

            if outcome.failed == 0 {
                self.state_store.complete_job(&job_id).await?;
            }

            Ok(outcome)
        } else {
            match node.node_type {
                NodeType::File => self.download_single_file(&api_client, &node, &target).await,
                _ => {
                    if download_opts.recursive {
                        let job_id = self.state_store.get_or_create_job(&source, &target).await?;
                        let completed_file_ids =
                            self.state_store.get_completed_file_ids(&job_id).await?;
                        let job_state = DownloadJobState {
                            job_id: &job_id,
                            completed_file_ids: &completed_file_ids,
                        };
                        let outcome = self
                            .download_container(
                                &api_client,
                                &node,
                                &target,
                                download_opts.velocity,
                                download_opts.include_rooms,
                                &job_state,
                            )
                            .await?;

                        if outcome.failed == 0 {
                            self.state_store.complete_job(&job_id).await?;
                        }

                        Ok(outcome)
                    } else {
                        Err(DcCmdError::InvalidArgument(
                            "Container download requires recursive flag".to_string(),
                        ))
                    }
                }
            }
        }
    }

    async fn search_query_files<A: NodesApi>(
        &self,
        api: &A,
        search_string: &str,
        parent_path: &str,
    ) -> Result<Vec<Node>, DcCmdError> {
        let parent_id = self
            .path_pagination
            .resolve_parent_id(api, Some(parent_path))
            .await?
            .ok_or_else(|| DcCmdError::InvalidPath(parent_path.to_string()))?;

        let nodes = self
            .path_pagination
            .search_nodes_all_pages(api, search_string, Some(parent_id), Some(0))
            .await?;

        Ok(nodes.get_files())
    }
}

impl Default for NodesDownloadService<OSFileSystem, NoopTransferStateStore> {
    fn default() -> Self {
        Self::new()
    }
}

fn split_remote_path(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::app::nodes::filesystem::{Filesystem, MockFilesystem, PathStyle};

    use super::{
        DownloadFailure, DownloadOutcome, NodesDownloadService, NoopTransferStateStore,
        RemoteNodeRef,
    };

    fn node(id: u64, name: &str, parent_path: &str) -> RemoteNodeRef {
        RemoteNodeRef {
            id,
            name: name.to_string(),
            parent_path: Some(parent_path.to_string()),
        }
    }

    #[test]
    fn test_resolve_single_file_target_unix() {
        let fs = MockFilesystem::with_directories(PathStyle::Unix, &["/tmp"]);
        let service = NodesDownloadService::with_dependencies(fs, NoopTransferStateStore);

        assert_eq!(
            service.resolve_single_file_target("/tmp", "report.pdf"),
            "/tmp/report.pdf"
        );
    }

    #[test]
    fn test_resolve_single_file_target_windows() {
        let fs = MockFilesystem::with_directories(PathStyle::Windows, &[r"C:\Downloads"]);
        let service = NodesDownloadService::with_dependencies(fs, NoopTransferStateStore);

        assert_eq!(
            service.resolve_single_file_target("C:/Downloads", "report.pdf"),
            r"C:\Downloads\report.pdf"
        );
    }

    #[test]
    fn test_resolve_targets_from_base_windows() {
        let fs = MockFilesystem::new(PathStyle::Windows);
        let service = NodesDownloadService::with_dependencies(fs, NoopTransferStateStore);
        let files = vec![
            node(11, "a.txt", "/team/room/"),
            node(22, "b.txt", "/team/room/"),
        ];

        let targets = service.resolve_targets_from_base(r"C:\downloads", &files);

        assert_eq!(targets.get(&11), Some(&r"C:\downloads\a.txt".to_string()));
        assert_eq!(targets.get(&22), Some(&r"C:\downloads\b.txt".to_string()));
    }

    #[test]
    fn test_plan_container_download_unix() {
        let fs = MockFilesystem::new(PathStyle::Unix);
        let service = NodesDownloadService::with_dependencies(fs, NoopTransferStateStore);
        let container = node(1, "parent", "/room/");
        let folders = vec![
            node(2, "sub", "/room/parent/"),
            node(3, "inner", "/room/parent/sub/"),
        ];
        let files = vec![
            node(10, "a.txt", "/room/parent/"),
            node(11, "b.txt", "/room/parent/sub/"),
        ];

        let plan = service
            .plan_container_download("/downloads", &container, &folders, &files)
            .unwrap();

        assert_eq!(plan.root_target, "/downloads/parent");
        assert_eq!(
            plan.directories,
            vec![
                "/downloads/parent".to_string(),
                "/downloads/parent/sub".to_string(),
                "/downloads/parent/sub/inner".to_string(),
            ]
        );
        assert_eq!(
            plan.file_targets,
            HashMap::from([
                (10, "/downloads/parent/a.txt".to_string()),
                (11, "/downloads/parent/sub/b.txt".to_string()),
            ])
        );
    }

    #[test]
    fn test_plan_container_download_windows() {
        let fs = MockFilesystem::new(PathStyle::Windows);
        let service = NodesDownloadService::with_dependencies(fs, NoopTransferStateStore);
        let container = node(1, "parent", "/room/");
        let folders = vec![
            node(2, "sub", "/room/parent/"),
            node(3, "inner", "/room/parent/sub/"),
        ];
        let files = vec![
            node(10, "a.txt", "/room/parent/"),
            node(11, "b.txt", "/room/parent/sub/"),
        ];

        let plan = service
            .plan_container_download(r"C:\downloads", &container, &folders, &files)
            .unwrap();

        assert_eq!(plan.root_target, r"C:\downloads\parent");
        assert_eq!(
            plan.directories,
            vec![
                r"C:\downloads\parent".to_string(),
                r"C:\downloads\parent\sub".to_string(),
                r"C:\downloads\parent\sub\inner".to_string(),
            ]
        );
        assert_eq!(
            plan.file_targets,
            HashMap::from([
                (10, r"C:\downloads\parent\a.txt".to_string()),
                (11, r"C:\downloads\parent\sub\b.txt".to_string()),
            ])
        );
    }

    #[test]
    fn test_plan_container_download_rejects_path_escape() {
        let fs = MockFilesystem::new(PathStyle::Unix);
        let service = NodesDownloadService::with_dependencies(fs, NoopTransferStateStore);
        let container = node(1, "parent", "/room/");
        let files = vec![node(10, "a.txt", "/other/")];

        let err = service
            .plan_container_download("/downloads", &container, &[], &files)
            .unwrap_err();

        assert!(matches!(
            err,
            crate::core::models::DcCmdError::InvalidPath(_)
        ));
    }

    #[test]
    fn test_create_directories_unix() {
        let fs = MockFilesystem::new(PathStyle::Unix);
        let service = NodesDownloadService::with_dependencies(fs, NoopTransferStateStore);

        service
            .create_directories(&["/tmp/a".to_string(), "/tmp/a/b".to_string()])
            .unwrap();

        assert!(service.fs.exists("/tmp/a"));
        assert!(service.fs.exists("/tmp/a/b"));
    }

    #[test]
    fn test_create_directories_windows() {
        let fs = MockFilesystem::new(PathStyle::Windows);
        let service = NodesDownloadService::with_dependencies(fs, NoopTransferStateStore);

        service
            .create_directories(&[r"C:\tmp\a".to_string(), r"C:\tmp\a\b".to_string()])
            .unwrap();

        assert!(service.fs.exists(r"C:\tmp\a"));
        assert!(service.fs.exists(r"C:\tmp\a\b"));
    }

    #[test]
    fn test_download_outcome_success_marks_no_failures() {
        let outcome = DownloadOutcome::success("/tmp", 2);

        assert_eq!(outcome.requested_total, 2);
        assert_eq!(outcome.succeeded, 2);
        assert_eq!(outcome.failed, 0);
        assert!(outcome.failures.is_empty());
        assert!(!outcome.partial);
    }

    #[test]
    fn test_download_outcome_from_failures_marks_partial() {
        let outcome = DownloadOutcome::from_failures(
            "/tmp",
            2,
            1,
            vec![DownloadFailure {
                node_id: Some(99),
                node_name: "broken.txt".to_string(),
                target: "/tmp/broken.txt".to_string(),
                reason: "network".to_string(),
            }],
        );

        assert_eq!(outcome.requested_total, 2);
        assert_eq!(outcome.succeeded, 1);
        assert_eq!(outcome.failed, 1);
        assert!(outcome.partial);
    }
}
