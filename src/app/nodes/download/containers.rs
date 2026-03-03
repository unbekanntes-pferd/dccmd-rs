use dco3::{
    nodes::{Node, NodeType, NodesSearchFilter, NodesSearchSortBy},
    SortOrder,
};
use tracing::{debug, info};

use crate::{
    app::nodes::{
        download::{
            DownloadJobState, DownloadOutcome, NodesDownloadService, RemoteNodeRef,
            TransferStateStore,
        },
        filesystem::Filesystem,
        progress::start_spinner,
    },
    core::models::DcCmdError,
};

use super::api::DownloadApi;

impl<F, S> NodesDownloadService<F, S>
where
    F: Filesystem,
    S: TransferStateStore,
{
    async fn filter_files_in_sub_rooms<A: DownloadApi>(
        &self,
        api: &A,
        parent_node: &Node,
        files: Vec<Node>,
    ) -> Result<Vec<Node>, DcCmdError> {
        debug!("Total file count: {}", files.len());
        let sub_room_paths = self.get_sub_room_paths(api, parent_node).await?;

        Ok(files
            .into_iter()
            .filter(|file| {
                !sub_room_paths.iter().any(|path| {
                    file.parent_path
                        .as_ref()
                        .is_some_and(|pp| pp.starts_with(path))
                })
            })
            .collect())
    }

    async fn get_sub_room_paths<A: DownloadApi>(
        &self,
        api: &A,
        parent_node: &Node,
    ) -> Result<Vec<String>, DcCmdError> {
        let rooms = self
            .search_nodes_all_pages_with_filter_sort(
                api,
                "*",
                Some(parent_node.id),
                None,
                NodesSearchFilter::is_room(),
                NodesSearchSortBy::parent_path(SortOrder::Asc),
            )
            .await?;

        Ok(rooms
            .get_rooms()
            .into_iter()
            .map(|room| {
                format!(
                    "{}{}/",
                    room.parent_path.unwrap_or_else(|| "/".into()),
                    room.name
                )
            })
            .collect())
    }

    async fn get_containers<A: DownloadApi>(
        &self,
        api: &A,
        parent_node: &Node,
        include_rooms: bool,
    ) -> Result<Vec<Node>, DcCmdError> {
        let filter = if include_rooms {
            NodesSearchFilter::is_types(vec![NodeType::Folder, NodeType::Room])
        } else {
            NodesSearchFilter::is_folder()
        };

        let nodes = self
            .search_nodes_all_pages_with_filter_sort(
                api,
                "*",
                Some(parent_node.id),
                Some(-1),
                filter,
                NodesSearchSortBy::parent_path(SortOrder::Asc),
            )
            .await?;

        let node_filter = if include_rooms {
            |node: &Node| node.node_type == NodeType::Folder || node.node_type == NodeType::Room
        } else {
            |node: &Node| node.node_type == NodeType::Folder
        };

        Ok(nodes
            .items
            .into_iter()
            .filter(node_filter)
            .collect::<Vec<_>>())
    }

    pub(super) async fn download_container<A: DownloadApi + Clone + Send + Sync + 'static>(
        &self,
        api: &A,
        node: &Node,
        target: &str,
        velocity: Option<u8>,
        include_rooms: bool,
        job_state: &DownloadJobState<'_>,
    ) -> Result<DownloadOutcome, DcCmdError> {
        info!("Attempting download of container {}.", node.name);
        info!("Target: {}", target);

        let progress_spinner = start_spinner(self.progress(), "Listing files and folders...");

        let folders = self.get_containers(api, node, include_rooms).await?;
        let files = self.get_files(api, node).await?;
        let files = if include_rooms {
            files
        } else {
            self.filter_files_in_sub_rooms(api, node, files).await?
        };

        progress_spinner.finish_and_clear();

        let container_ref = RemoteNodeRef::from(node);
        let folder_refs = folders.iter().map(RemoteNodeRef::from).collect::<Vec<_>>();
        let file_refs = files.iter().map(RemoteNodeRef::from).collect::<Vec<_>>();

        let plan =
            self.plan_container_download(target, &container_ref, &folder_refs, &file_refs)?;
        self.create_directories(&plan.directories)?;

        let outcome = self
            .download_files_with_state(
                api,
                files,
                plan.file_targets,
                velocity,
                &plan.root_target,
                job_state,
            )
            .await?;

        info!("Download of container {} complete.", node.name);
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use dco3::{
        models::{Range, RangedItems},
        nodes::{models::NodeType, Node, NodeList},
        ListAllParams,
    };
    use tokio::io::{AsyncWrite, AsyncWriteExt};

    use crate::{
        app::nodes::{
            api::NodesApi,
            download::{api::DownloadApi, NodesDownloadService, NoopTransferStateStore},
            filesystem::OSFileSystem,
        },
        core::models::DcCmdError,
    };

    struct MockDownloadApi {
        responses: Mutex<Vec<NodeList>>,
        search_calls: Mutex<u32>,
    }

    impl MockDownloadApi {
        fn new(responses: Vec<NodeList>) -> Self {
            Self {
                responses: Mutex::new(responses),
                search_calls: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl NodesApi for MockDownloadApi {
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

            let mut responses = self.responses.lock().expect("lock poisoned");
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
            _node: &Node,
            writer: &'w mut (dyn AsyncWrite + Send + Unpin),
            _callback: Option<dco3::nodes::models::DownloadProgressCallback>,
        ) -> Result<(), DcCmdError> {
            writer.write_all(b"").await.map_err(|_| DcCmdError::IoError)
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
            size: None,
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

    #[tokio::test]
    async fn test_filter_files_in_sub_rooms_excludes_nested_room_files() {
        let parent = node(1, "root", NodeType::Room, "/");
        let files = vec![
            node(10, "root-a.txt", NodeType::File, "/root/"),
            node(11, "nested.txt", NodeType::File, "/root/sub-room/"),
        ];

        let api = MockDownloadApi::new(vec![node_list(
            vec![node(50, "sub-room", NodeType::Room, "/root/")],
            1,
        )]);
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let filtered = service
            .filter_files_in_sub_rooms(&api, &parent, files)
            .await
            .unwrap();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 10);
    }

    #[tokio::test]
    async fn test_filter_files_in_sub_rooms_fetches_additional_pages() {
        let parent = node(1, "root", NodeType::Room, "/");
        let files = vec![
            node(10, "root-a.txt", NodeType::File, "/root/"),
            node(11, "nested-a.txt", NodeType::File, "/root/sub-room-a/"),
            node(12, "nested-b.txt", NodeType::File, "/root/sub-room-b/"),
        ];

        let page1 = node_list(vec![node(50, "sub-room-a", NodeType::Room, "/root/")], 700);
        let page2 = node_list(vec![node(51, "sub-room-b", NodeType::Room, "/root/")], 700);
        let api = MockDownloadApi::new(vec![page1, page2]);
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let filtered = service
            .filter_files_in_sub_rooms(&api, &parent, files)
            .await
            .unwrap();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 10);
        assert_eq!(*api.search_calls.lock().expect("lock poisoned"), 2);
    }

    #[tokio::test]
    async fn test_get_containers_excludes_rooms_when_include_rooms_is_false() {
        let parent = node(1, "root", NodeType::Room, "/");
        let api = MockDownloadApi::new(vec![node_list(
            vec![
                node(10, "folder-a", NodeType::Folder, "/root/"),
                node(11, "room-a", NodeType::Room, "/root/"),
            ],
            2,
        )]);
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let containers = service.get_containers(&api, &parent, false).await.unwrap();

        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].node_type, NodeType::Folder);
        assert_eq!(containers[0].id, 10);
    }

    #[tokio::test]
    async fn test_get_containers_fetches_next_page() {
        let parent = node(1, "root", NodeType::Room, "/");
        let page1 = node_list(vec![node(10, "folder-a", NodeType::Folder, "/root/")], 700);
        let page2 = node_list(vec![node(11, "folder-b", NodeType::Folder, "/root/")], 700);
        let api = MockDownloadApi::new(vec![page1, page2]);
        let service = NodesDownloadService::with_dependencies(OSFileSystem, NoopTransferStateStore);

        let containers = service.get_containers(&api, &parent, false).await.unwrap();

        assert_eq!(containers.len(), 2);
        assert_eq!(*api.search_calls.lock().expect("lock poisoned"), 2);
    }
}
