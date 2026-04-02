use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;
use dco3::{
    models::{Range, RangedItems},
    nodes::{models::NodeType, Node, NodeList},
    ListAllParams,
};

use crate::{
    app::{nodes::api::NodesApi, users::api::UsersApi},
    command::CreateContainerType,
    core::models::{DcCmdError, ListOptions},
};

use super::{DeleteNodesPreparation, NodesService};

type CreatedFolderRecord = (String, u64, Option<u8>, Option<String>);
type CreatedRoomRecord = (String, u64, u8, bool, Option<Vec<u64>>);

struct MockNodesApi {
    node_by_path: HashMap<String, Node>,
    user_by_name: HashMap<String, u64>,
    search_result: NodeList,
    nodes_result: NodeList,
    search_calls: Mutex<u32>,
    get_nodes_calls: Mutex<u32>,
    deleted_node_ids: Mutex<Vec<u64>>,
    deleted_batch_ids: Mutex<Vec<Vec<u64>>>,
    copied_batches: Mutex<Vec<(Vec<u64>, u64)>>,
    created_folders: Mutex<Vec<CreatedFolderRecord>>,
    created_rooms: Mutex<Vec<CreatedRoomRecord>>,
}

impl MockNodesApi {
    fn new(
        node_by_path: HashMap<String, Node>,
        user_by_name: HashMap<String, u64>,
        search_result: NodeList,
    ) -> Self {
        Self {
            node_by_path,
            user_by_name,
            search_result,
            nodes_result: empty_node_list(),
            search_calls: Mutex::new(0),
            get_nodes_calls: Mutex::new(0),
            deleted_node_ids: Mutex::new(Vec::new()),
            deleted_batch_ids: Mutex::new(Vec::new()),
            copied_batches: Mutex::new(Vec::new()),
            created_folders: Mutex::new(Vec::new()),
            created_rooms: Mutex::new(Vec::new()),
        }
    }

    fn with_nodes_result(mut self, nodes_result: NodeList) -> Self {
        self.nodes_result = nodes_result;
        self
    }
}

#[async_trait]
impl NodesApi for MockNodesApi {
    async fn get_node_from_path(&self, node_path: &str) -> Result<Option<Node>, DcCmdError> {
        Ok(self.node_by_path.get(node_path).cloned())
    }

    async fn get_nodes(
        &self,
        _parent_id: Option<u64>,
        _managed: Option<bool>,
        _params: Option<ListAllParams>,
    ) -> Result<NodeList, DcCmdError> {
        let mut calls = self.get_nodes_calls.lock().expect("lock poisoned");
        *calls += 1;
        Ok(self.nodes_result.clone())
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
        Ok(self.search_result.clone())
    }

    async fn delete_node(&self, node_id: u64) -> Result<(), DcCmdError> {
        self.deleted_node_ids
            .lock()
            .expect("lock poisoned")
            .push(node_id);
        Ok(())
    }

    async fn delete_nodes(&self, node_ids: Vec<u64>) -> Result<(), DcCmdError> {
        self.deleted_batch_ids
            .lock()
            .expect("lock poisoned")
            .push(node_ids);
        Ok(())
    }

    async fn copy_nodes(
        &self,
        node_ids: Vec<u64>,
        target_parent_id: u64,
    ) -> Result<(), DcCmdError> {
        self.copied_batches
            .lock()
            .expect("lock poisoned")
            .push((node_ids, target_parent_id));
        Ok(())
    }

    async fn create_folder(
        &self,
        node_name: &str,
        parent_id: u64,
        classification: Option<u8>,
        notes: Option<String>,
    ) -> Result<(), DcCmdError> {
        self.created_folders.lock().expect("lock poisoned").push((
            node_name.to_string(),
            parent_id,
            classification,
            notes,
        ));
        Ok(())
    }

    async fn create_room(
        &self,
        node_name: &str,
        parent_id: u64,
        classification: u8,
        inherit_permissions: bool,
        admin_ids: Option<Vec<u64>>,
    ) -> Result<(), DcCmdError> {
        self.created_rooms.lock().expect("lock poisoned").push((
            node_name.to_string(),
            parent_id,
            classification,
            inherit_permissions,
            admin_ids,
        ));
        Ok(())
    }
}

#[async_trait]
impl UsersApi for MockNodesApi {
    async fn find_user_id_by_username(&self, user_name: &str) -> Result<u64, DcCmdError> {
        self.user_by_name.get(user_name).copied().ok_or_else(|| {
            DcCmdError::InvalidArgument(format!("No user found with username: {user_name}"))
        })
    }
}

fn empty_node_list() -> NodeList {
    RangedItems {
        range: Range {
            offset: 0,
            limit: 500,
            total: 0,
        },
        items: Vec::new(),
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

fn node_list(items: Vec<Node>) -> NodeList {
    let total = items.len() as u64;
    RangedItems {
        range: Range {
            offset: 0,
            limit: 500,
            total,
        },
        items,
    }
}

fn node_list_with_total(items: Vec<Node>, total: u64) -> NodeList {
    RangedItems {
        range: Range {
            offset: 0,
            limit: 500,
            total,
        },
        items,
    }
}

#[tokio::test]
async fn test_list_nodes_uses_get_nodes_for_non_search() {
    let api = MockNodesApi::new(
        HashMap::from([(String::from("/test/"), node(1, "test", NodeType::Room, "/"))]),
        HashMap::new(),
        empty_node_list(),
    )
    .with_nodes_result(node_list(vec![node(
        11,
        "file.txt",
        NodeType::File,
        "/test/",
    )]));
    let service = NodesService::new(api);

    let result = service
        .list_nodes(
            "example.com/test",
            "https://example.com",
            false,
            &ListOptions::new(None, None, None, false, false),
        )
        .await
        .unwrap();

    assert_eq!(result.node_path.as_deref(), Some("/test/"));
    assert_eq!(result.list.items.len(), 1);
    assert_eq!(
        *service.api.get_nodes_calls.lock().expect("lock poisoned"),
        1
    );
    assert_eq!(*service.api.search_calls.lock().expect("lock poisoned"), 0);
}

#[tokio::test]
async fn test_list_nodes_uses_search_for_wildcard() {
    let api = MockNodesApi::new(
        HashMap::from([(String::from("/test/"), node(1, "test", NodeType::Room, "/"))]),
        HashMap::new(),
        node_list(vec![node(11, "file.txt", NodeType::File, "/test/")]),
    );
    let service = NodesService::new(api);

    let result = service
        .list_nodes(
            "example.com/test/*",
            "https://example.com",
            false,
            &ListOptions::new(None, None, None, false, false),
        )
        .await
        .unwrap();

    assert_eq!(result.node_path.as_deref(), Some("/test/*/"));
    assert_eq!(result.list.items.len(), 1);
    assert_eq!(*service.api.search_calls.lock().expect("lock poisoned"), 1);
    assert_eq!(
        *service.api.get_nodes_calls.lock().expect("lock poisoned"),
        0
    );
}

#[tokio::test]
async fn test_list_nodes_all_fetches_next_page() {
    let api = MockNodesApi::new(
        HashMap::from([(String::from("/test/"), node(1, "test", NodeType::Room, "/"))]),
        HashMap::new(),
        empty_node_list(),
    )
    .with_nodes_result(node_list_with_total(
        vec![node(11, "file.txt", NodeType::File, "/test/")],
        700,
    ));
    let service = NodesService::new(api);

    let result = service
        .list_nodes(
            "example.com/test",
            "https://example.com",
            false,
            &ListOptions::new(None, None, None, true, false),
        )
        .await
        .unwrap();

    assert_eq!(result.list.items.len(), 2);
    assert_eq!(
        *service.api.get_nodes_calls.lock().expect("lock poisoned"),
        2
    );
}

#[tokio::test]
async fn test_prepare_delete_requires_recursive_for_search_query() {
    let api = MockNodesApi::new(
        HashMap::from([(String::from("/test/"), node(1, "test", NodeType::Room, "/"))]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let prep = service
        .prepare_delete("example.com/test/*", "https://example.com", false)
        .await
        .unwrap();

    assert!(matches!(
        prep,
        DeleteNodesPreparation::InvalidSearchRequiresRecursive
    ));
}

#[tokio::test]
async fn test_prepare_delete_search_filters_out_rooms() {
    let api = MockNodesApi::new(
        HashMap::from([(String::from("/test/"), node(1, "test", NodeType::Room, "/"))]),
        HashMap::new(),
        node_list(vec![
            node(11, "file-1.txt", NodeType::File, "/test/"),
            node(22, "room-2", NodeType::Room, "/test/"),
            node(33, "folder-3", NodeType::Folder, "/test/"),
        ]),
    );
    let service = NodesService::new(api);

    let prep = service
        .prepare_delete("example.com/test/*", "https://example.com", true)
        .await
        .unwrap();

    match prep {
        DeleteNodesPreparation::Search { node_ids } => {
            assert_eq!(node_ids, vec![11, 33]);
        }
        _ => panic!("expected search delete preparation"),
    }
}

#[tokio::test]
async fn test_prepare_delete_requires_recursive_for_container() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/test/folder/"),
            node(42, "folder", NodeType::Folder, "/test/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let prep = service
        .prepare_delete("example.com/test/folder", "https://example.com", false)
        .await
        .unwrap();

    assert!(matches!(
        prep,
        DeleteNodesPreparation::ContainerRequiresRecursive
    ));
}

#[tokio::test]
async fn test_prepare_delete_returns_single_file_node() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/test/file.txt/"),
            node(42, "file.txt", NodeType::File, "/test/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let prep = service
        .prepare_delete("example.com/test/file.txt", "https://example.com", false)
        .await
        .unwrap();

    match prep {
        DeleteNodesPreparation::SingleNode {
            node_id,
            node_name,
            node_type,
        } => {
            assert_eq!(node_id, 42);
            assert_eq!(node_name, "file.txt");
            assert_eq!(node_type, NodeType::File);
        }
        _ => panic!("expected single node delete preparation"),
    }
}

#[tokio::test]
async fn test_copy_nodes_single_source_node() {
    let api = MockNodesApi::new(
        HashMap::from([
            (
                String::from("/test/file.txt/"),
                node(11, "file.txt", NodeType::File, "/test/"),
            ),
            (
                String::from("/target/"),
                node(99, "target", NodeType::Folder, "/"),
            ),
        ]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let result = service
        .copy_nodes(
            "example.com/test/file.txt",
            "/target/",
            "https://example.com",
        )
        .await
        .unwrap();

    assert_eq!(result.count_nodes, 1);
    assert_eq!(result.source_parent_path, "/test/");
    assert_eq!(result.target_path, "/target/");
    assert_eq!(
        service
            .api
            .copied_batches
            .lock()
            .expect("lock poisoned")
            .as_slice(),
        &[(vec![11], 99)]
    );
}

#[tokio::test]
async fn test_copy_nodes_search_source() {
    let api = MockNodesApi::new(
        HashMap::from([
            (String::from("/test/"), node(1, "test", NodeType::Room, "/")),
            (
                String::from("/target/"),
                node(99, "target", NodeType::Folder, "/"),
            ),
        ]),
        HashMap::new(),
        node_list(vec![
            node(11, "file-1.txt", NodeType::File, "/test/"),
            node(22, "file-2.txt", NodeType::File, "/test/"),
        ]),
    );
    let service = NodesService::new(api);

    let result = service
        .copy_nodes("example.com/test/*", "/target/", "https://example.com")
        .await
        .unwrap();

    assert_eq!(result.count_nodes, 2);
    assert_eq!(result.source_parent_path, "/test/");
    assert_eq!(
        service
            .api
            .copied_batches
            .lock()
            .expect("lock poisoned")
            .as_slice(),
        &[(vec![11, 22], 99)]
    );
}

#[tokio::test]
async fn test_copy_nodes_resolves_full_url_target_path() {
    let api = MockNodesApi::new(
        HashMap::from([
            (
                String::from("/test/file.txt/"),
                node(11, "file.txt", NodeType::File, "/test/"),
            ),
            (
                String::from("/e2e/test-target/"),
                node(99, "test-target", NodeType::Folder, "/e2e/"),
            ),
        ]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let result = service
        .copy_nodes(
            "example.com/test/file.txt",
            "https://example.com/e2e/test-target",
            "https://example.com",
        )
        .await
        .unwrap();

    assert_eq!(result.count_nodes, 1);
    assert_eq!(result.source_parent_path, "/test/");
    assert_eq!(result.target_path, "https://example.com/e2e/test-target");
    assert_eq!(
        service
            .api
            .copied_batches
            .lock()
            .expect("lock poisoned")
            .as_slice(),
        &[(vec![11], 99)]
    );
}

#[tokio::test]
async fn test_copy_nodes_errors_when_source_missing() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/target/"),
            node(99, "target", NodeType::Folder, "/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let result = service
        .copy_nodes(
            "example.com/test/file.txt",
            "/target/",
            "https://example.com",
        )
        .await;

    assert!(matches!(
        result,
        Err(DcCmdError::InvalidPath(path)) if path == "example.com/test/file.txt"
    ));
}

#[tokio::test]
async fn test_copy_nodes_errors_when_target_missing() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/test/file.txt/"),
            node(11, "file.txt", NodeType::File, "/test/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let result = service
        .copy_nodes(
            "example.com/test/file.txt",
            "/target/",
            "https://example.com",
        )
        .await;

    assert!(matches!(
        result,
        Err(DcCmdError::InvalidPath(path)) if path == "/target/"
    ));
}

#[tokio::test]
async fn test_create_folder_success() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/teams/"),
            node(7, "teams", NodeType::Room, "/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let message = service
        .create_container(
            "example.com/teams/folder-a",
            "https://example.com",
            CreateContainerType::Folder,
            Some(4),
            Some("folder notes".to_string()),
            None,
            false,
        )
        .await
        .unwrap();

    assert_eq!(message, "Folder folder-a created.");
    assert_eq!(
        service
            .api
            .created_folders
            .lock()
            .expect("lock poisoned")
            .as_slice(),
        &[(
            "folder-a".to_string(),
            7,
            Some(4),
            Some("folder notes".to_string())
        )]
    );
}

#[tokio::test]
async fn test_create_folder_rejects_room_only_options() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/teams/"),
            node(7, "teams", NodeType::Room, "/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let err = service
        .create_container(
            "example.com/teams/folder-b",
            "https://example.com",
            CreateContainerType::Folder,
            None,
            None,
            Some(vec!["alice".to_string()]),
            true,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        DcCmdError::InvalidArgument(msg)
        if msg == "Room-only options are not supported when creating folders."
    ));
}

#[tokio::test]
async fn test_create_room_inherit_defaults_to_true_without_admin_users() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/teams/"),
            node(7, "teams", NodeType::Room, "/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let message = service
        .create_container(
            "example.com/teams/room-a",
            "https://example.com",
            CreateContainerType::Room,
            None,
            None,
            None,
            false,
        )
        .await
        .unwrap();

    assert_eq!(message, "Room room-a created.");
    assert_eq!(
        service
            .api
            .created_rooms
            .lock()
            .expect("lock poisoned")
            .as_slice(),
        &[("room-a".to_string(), 7, 2, true, None)]
    );
}

#[tokio::test]
async fn test_create_room_inherit_follows_flag_with_admin_users() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/teams/"),
            node(7, "teams", NodeType::Room, "/"),
        )]),
        HashMap::from([(String::from("alice"), 55_u64)]),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let message = service
        .create_container(
            "example.com/teams/room-b",
            "https://example.com",
            CreateContainerType::Room,
            Some(3),
            None,
            Some(vec!["alice".to_string()]),
            false,
        )
        .await
        .unwrap();

    assert_eq!(message, "Room room-b created.");
    assert_eq!(
        service
            .api
            .created_rooms
            .lock()
            .expect("lock poisoned")
            .as_slice(),
        &[("room-b".to_string(), 7, 3, false, Some(vec![55]))]
    );
}

#[tokio::test]
async fn test_create_room_rejects_notes() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/teams/"),
            node(7, "teams", NodeType::Room, "/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let err = service
        .create_container(
            "example.com/teams/room-c",
            "https://example.com",
            CreateContainerType::Room,
            None,
            Some("room notes".to_string()),
            None,
            false,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        DcCmdError::InvalidArgument(msg)
        if msg == "Notes are only supported when creating folders."
    ));
}

#[tokio::test]
async fn test_create_room_requires_room_parent() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/teams/"),
            node(7, "teams", NodeType::Folder, "/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let err = service
        .create_container(
            "example.com/teams/room-d",
            "https://example.com",
            CreateContainerType::Room,
            None,
            None,
            None,
            false,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        DcCmdError::InvalidPath(path) if path == "example.com/teams/room-d"
    ));
}

#[tokio::test]
async fn test_create_room_errors_when_admin_user_not_found() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/teams/"),
            node(7, "teams", NodeType::Room, "/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let err = service
        .create_container(
            "example.com/teams/room-e",
            "https://example.com",
            CreateContainerType::Room,
            None,
            None,
            Some(vec!["alice".to_string()]),
            true,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        DcCmdError::InvalidArgument(msg)
        if msg == "No user found with username: alice"
    ));
}

#[tokio::test]
async fn test_create_room_errors_when_admin_user_list_is_empty() {
    let api = MockNodesApi::new(
        HashMap::from([(
            String::from("/teams/"),
            node(7, "teams", NodeType::Room, "/"),
        )]),
        HashMap::new(),
        empty_node_list(),
    );
    let service = NodesService::new(api);

    let err = service
        .create_container(
            "example.com/teams/room-f",
            "https://example.com",
            CreateContainerType::Room,
            None,
            None,
            Some(Vec::new()),
            true,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        DcCmdError::InvalidArgument(msg) if msg == "No valid admin users provided."
    ));
}
