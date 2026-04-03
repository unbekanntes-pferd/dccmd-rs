pub const GROUPS_LIST: &str = r#"List groups on the fixed DRACOON target.

Purpose:
- Use this tool to list groups available on the startup target.
- The server target is fixed at startup, so no target argument is required.

Arguments:
- `filter`: optional string filter passed through to the group list query.
- `offset`: optional unsigned integer. Default `0`.
- `limit`: optional unsigned integer. Default `500`.
- `all`: optional boolean. Default `false`. When `true`, the server keeps requesting pages until all results are fetched.

Pagination:
- `offset` and `limit` control the first request page.
- `all: true` keeps paging until the reported total is fetched.

Examples:
- List the first page:
  `{}`
- Filtered full listing:
  `{"filter": "team", "all": true}`

Common failures:
- Using an invalid filter query.
- Missing stored auth for the fixed startup target.
"#;

pub const GROUPS_CREATE: &str = r#"Create a group on the fixed DRACOON target.

Purpose:
- Use this tool to create one group on the startup target.
- The server target is fixed at startup, so no target argument is required.

Arguments:
- `name`: required string.

Examples:
- Create a group:
  `{"name": "Engineering"}`

Common failures:
- Missing required group name.
- Duplicate group name or conflicting group settings.
- Missing stored auth for the fixed startup target.
"#;

pub const GROUPS_GET_USERS: &str = r#"List users of one group on the fixed DRACOON target.

Purpose:
- Use this tool to list users assigned to one group on the startup target.
- The server target is fixed at startup, so no target argument is required.

Selector rules:
- Provide exactly one selector: `group_id` or `group_name`.

Arguments:
- `group_id`: optional unsigned integer. Preferred stable selector.
- `group_name`: optional string exact group-name selector.
- `filter`: optional string filter passed through to the group-user list query.
- `offset`: optional unsigned integer. Default `0`.
- `limit`: optional unsigned integer. Default `500`.
- `all`: optional boolean. Default `false`. When `true`, the server keeps requesting pages until all results are fetched.

Examples:
- List users by group id:
  `{"group_id": 12345}`
- Filtered full listing by group name:
  `{"group_name": "Engineering", "filter": "alice", "all": true}`

Common failures:
- Providing both `group_id` and `group_name`.
- Providing neither `group_id` nor `group_name`.
- No group matches the provided selector.
- Missing stored auth for the fixed startup target.
"#;

pub const GROUPS_ADD_USERS: &str = r#"Add users to one group on the fixed DRACOON target.

Purpose:
- Use this tool to add one or more users to one group on the startup target.
- The server target is fixed at startup, so no target argument is required.

Selector rules:
- Provide exactly one group selector: `group_id` or `group_name`.
- Provide at least one user selector source: `user_ids` or `usernames`.

Arguments:
- `group_id`: optional unsigned integer. Preferred stable selector.
- `group_name`: optional string exact group-name selector.
- `user_ids`: optional list of unsigned integers.
- `usernames`: optional list of usernames. Exact-match lookup is used for each username.

Examples:
- Add by ids:
  `{"group_id": 12345, "user_ids": [7, 8]}`
- Add by usernames:
  `{"group_name": "Engineering", "usernames": ["alice", "bob"]}`

Common failures:
- Providing both `group_id` and `group_name`.
- Providing neither `group_id` nor `group_name`.
- Providing no user ids and no usernames.
- No group or user matches the provided selector.
- Missing stored auth for the fixed startup target.
"#;
