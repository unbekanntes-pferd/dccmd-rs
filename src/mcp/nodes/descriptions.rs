pub const NODES_LIST: &str = r#"List child nodes under a DRACOON container on the fixed server target.

Purpose:
- Use this tool to list files, folders, and rooms below a container on the startup target.
- Prefer `node_id` for stable automation. `path` is a convenience fallback.

Selector rules:
- Provide exactly one selector: `node_id` or `path`.
- `node_id` is preferred.
- `path` is a DRACOON path relative to the fixed startup target, for example `/Projects/Finance`.
- Use `path: "/"` to list the root level.

Arguments:
- `node_id`: optional unsigned integer. Preferred identifier for the parent container.
- `path`: optional string. Relative DRACOON path on the startup target.
- `filter`: optional structured filter. Only one filter object is supported.
- `offset`: optional unsigned integer. Default `0`.
- `limit`: optional unsigned integer. Default `500`.
- `all`: optional boolean. Default `false`. When `true`, the server keeps requesting pages until all results are fetched.
- `managed`: optional boolean. Default `false`. When `true`, list nodes in managed-room context where supported.

Allowed filters:
- `name` with `equals` or `contains`, string value.
- `type` with an exact list of `file`, `folder`, or `room`.
- `encrypted` with exact boolean value.
- `branch_version` with `gte` or `lte`, unsigned integer value.
- `created_at` with `gte` or `lte`, RFC3339 timestamp string.
- `updated_at` with `gte` or `lte`, RFC3339 timestamp string.
- `reference_id` with exact unsigned integer value.

Pagination:
- `offset` and `limit` control the first request page.
- `all: true` keeps paging until the reported total is fetched.
- When `all: true`, progress notifications are emitted when the client supplied a progress token.

Progress behavior:
- Long multi-page listings report progress with the number of fetched items and total when known.
- If the client did not supply a progress token, the listing still completes normally without notifications.

Examples:
- Minimal by node id:
  `{"node_id": 12345}`
- Filtered full listing by path:
  `{"path": "/Projects/Finance", "filter": {"field": "name", "op": "contains", "value": "report"}, "all": true}`

Common failures:
- Providing both `node_id` and `path`.
- Providing neither `node_id` nor `path`.
- Using an unsupported filter field or operator.
- Listing below a file node instead of a container.
- Missing stored auth for the fixed startup target.
"#;

pub const NODES_DOWNLOAD: &str = r#"Download a node tree from the fixed DRACOON target into the local workspace.

Purpose:
- Use this tool to download one file or one container from the startup target.
- Prefer `node_id` for the remote selector. `path` is optional.

Selector rules:
- Provide exactly one remote selector: `node_id` or `path`.
- `path` is relative to the fixed startup target, for example `/Projects/Finance/report.pdf`.
- Local `target_path` is always interpreted relative to the server workspace root unless it is an absolute path inside that same root.

Arguments:
- `node_id`: optional unsigned integer. Preferred remote selector.
- `path`: optional string. DRACOON path relative to the startup target.
- `target_path`: required string. Local output path inside the server workspace root.
- `recursive`: optional boolean. Default `false`. Required for container downloads.
- `velocity`: optional integer from `1` to `10`. Controls concurrency multiplier.
- `include_rooms`: optional boolean. Default `false`. Include nested rooms when downloading container trees.

Safety rules:
- The server rejects local target paths that escape the workspace root.
- The tool never prompts for auth or encryption secrets.
- Encrypted downloads require a stored encryption secret for the startup target or a startup `--encryption-password`.

Progress behavior:
- File and container downloads emit progress notifications when the client supplied a progress token.
- Progress covers file transfer and recursive folder creation.

Examples:
- Download one file by id:
  `{"node_id": 12345, "target_path": "downloads/report.pdf"}`
- Download one folder recursively by path:
  `{"path": "/Projects/Finance", "target_path": "downloads/finance", "recursive": true, "include_rooms": false}`

Common failures:
- Providing both `node_id` and `path`.
- Missing `recursive` for a folder or room download.
- Local target path escapes the workspace root.
- Missing stored auth or missing encryption secret for an encrypted node.
"#;

pub const NODES_READ: &str = r#"Read a remote text file from the fixed DRACOON target into the MCP response.

Purpose:
- Use this tool to read one remote file from the startup target without writing it to the local workspace.
- Prefer `node_id` for the remote selector. `path` is optional.

Selector rules:
- Provide exactly one remote selector: `node_id` or `path`.
- `path` is relative to the fixed startup target, for example `/Projects/Finance/report.json`.

Arguments:
- `node_id`: optional unsigned integer. Preferred remote selector.
- `path`: optional string. DRACOON path relative to the startup target.

Safety rules:
- The tool only returns recognized text files.
- The maximum supported file size is `1048576` bytes.
- The tool requires valid UTF-8 content.
- Encrypted files require a stored encryption secret for the startup target or a startup `--encryption-password`.

Examples:
- Read one file by id:
  `{"node_id": 12345}`
- Read one file by path:
  `{"path": "/Projects/Finance/report.json"}`

Common failures:
- Providing both `node_id` and `path`.
- Providing neither `node_id` nor `path`.
- Selector resolves to a folder or room instead of a file.
- File type is not recognized as text.
- File size exceeds `1048576` bytes.
- File content is not valid UTF-8.
- Missing stored auth or missing encryption secret for an encrypted node.
"#;

pub const NODES_UPLOAD: &str = r#"Upload a local file or directory from the workspace into the fixed DRACOON target.

Purpose:
- Use this tool to upload one file or one local directory into a DRACOON parent container on the startup target.
- Prefer `parent_node_id` for the destination selector. `parent_path` is optional.

Selector rules:
- Provide exactly one destination selector: `parent_node_id` or `parent_path`.
- `parent_path` is relative to the fixed startup target, for example `/Projects/Finance`.
- `source_path` is always interpreted relative to the server workspace root unless it is an absolute path inside that same root.

Arguments:
- `source_path`: required string. Local file or directory path inside the workspace root.
- `parent_node_id`: optional unsigned integer. Preferred destination selector.
- `parent_path`: optional string. DRACOON parent path relative to the startup target.
- `overwrite`: optional boolean. Default `false`.
- `keep_share_links`: optional boolean. Default `false`.
- `recursive`: optional boolean. Default `false`. Required for directory uploads.
- `skip_root`: optional boolean. Default `false`. When uploading a directory recursively, skip creating the top-level local directory on the remote side.
- `share`: optional boolean. Default `false`. Create a download share for uploaded files when supported.
- `classification`: optional integer from `1` to `4`.
- `velocity`: optional integer from `1` to `10`.
- `share_password`: optional string. Password for created download shares.

Safety rules:
- The server rejects local source paths that escape the workspace root.
- The tool never prompts for auth or encryption secrets.
- Encrypted destination parents require a stored encryption secret for the startup target or a startup `--encryption-password`.

Progress behavior:
- File uploads, recursive folder creation, and multi-file uploads emit progress notifications when the client supplied a progress token.

Examples:
- Upload one file by parent id:
  `{"source_path": "artifacts/report.pdf", "parent_node_id": 98765}`
- Upload one directory recursively by path:
  `{"source_path": "build/output", "parent_path": "/Projects/Finance", "recursive": true, "skip_root": true, "overwrite": true}`

Common failures:
- Providing both `parent_node_id` and `parent_path`.
- Missing `recursive` for a directory upload.
- Local source path escapes the workspace root.
- Missing stored auth or missing encryption secret for an encrypted destination parent.
"#;

pub const NODES_COPY: &str = r#"Copy existing nodes inside the fixed DRACOON target.

Purpose:
- Use this tool to copy one remote node into another parent container on the startup target.
- Prefer node ids for both selectors.

Selector rules:
- Provide exactly one source selector: `source_node_id` or `source_path`.
- Provide exactly one target selector: `target_parent_node_id` or `target_parent_path`.
- Paths are relative to the fixed startup target.

Arguments:
- `source_node_id`: optional unsigned integer. Preferred source selector.
- `source_path`: optional string. Source DRACOON path.
- `target_parent_node_id`: optional unsigned integer. Preferred destination parent selector.
- `target_parent_path`: optional string. Destination parent DRACOON path.

Progress behavior:
- This tool does not emit incremental progress because the DRACOON copy call is single-shot.

Examples:
- Copy by ids:
  `{"source_node_id": 12345, "target_parent_node_id": 98765}`
- Copy by path into another container path:
  `{"source_path": "/Projects/Finance/report.pdf", "target_parent_path": "/Projects/Archive"}`

Common failures:
- Missing or ambiguous selectors.
- Destination selector resolves to a file instead of a container.
- Missing stored auth for the fixed startup target.
"#;

pub const NODES_MKDIR: &str = r#"Create a folder or room inside the fixed DRACOON target.

Purpose:
- Use this tool to create a new folder or room below an existing parent container on the startup target.
- Prefer `parent_node_id` for stable automation.

Selector rules:
- Provide exactly one parent selector: `parent_node_id` or `parent_path`.
- Paths are relative to the fixed startup target.

Arguments:
- `parent_node_id`: optional unsigned integer. Preferred parent selector.
- `parent_path`: optional string. Parent DRACOON path.
- `name`: required string. New node name.
- `type`: required enum. Either `folder` or `room`.
- `classification`: optional integer from `1` to `4`. For rooms, DRACOON defaults apply when omitted.
- `notes`: optional string. Supported for folders only.
- `admin_users`: optional list of usernames. Supported for rooms only.
- `inherit_permissions`: optional boolean. Room-only option.

Safety rules:
- The tool never prompts.
- Folder creation rejects room-only options.
- Room creation rejects folder-only options and requires a room parent where DRACOON requires it.

Progress behavior:
- This tool does not emit incremental progress because creation is a single API call plus admin lookup.

Examples:
- Create a folder:
  `{"parent_node_id": 98765, "name": "Q2 Reports", "type": "folder"}`
- Create a room with admins:
  `{"parent_path": "/Projects", "name": "Finance Room", "type": "room", "admin_users": ["alice", "bob"], "inherit_permissions": true}`

Common failures:
- Missing or ambiguous parent selectors.
- Using room-only options with `type: "folder"`.
- Using `notes` with `type: "room"`.
- Missing stored auth for the fixed startup target.
"#;

pub const NODES_RM: &str = r#"Delete nodes from the fixed DRACOON target.

Purpose:
- Use this tool to delete one file, folder, or room from the startup target.
- This tool is only registered when the server was started with `--allow-destructive`.

Selector rules:
- Provide exactly one selector: `node_id` or `path`.
- `node_id` is preferred.
- `path` is relative to the fixed startup target.

Arguments:
- `node_id`: optional unsigned integer. Preferred selector.
- `path`: optional string. DRACOON node path.
- `recursive`: optional boolean. Default `false`. Required for folders and rooms.

Safety rules:
- The tool is absent unless destructive mode was enabled at startup.
- The tool never prompts for confirmation.

Progress behavior:
- This tool does not emit incremental progress because deletion is a single API call after preflight validation.

Examples:
- Delete one file by id:
  `{"node_id": 12345}`
- Delete one folder recursively by path:
  `{"path": "/Projects/Archive", "recursive": true}`

Common failures:
- Starting the server without `--allow-destructive`.
- Missing or ambiguous selectors.
- Deleting a folder or room without `recursive: true`.
- Missing stored auth for the fixed startup target.
"#;
