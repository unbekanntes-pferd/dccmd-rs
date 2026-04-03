pub const USERS_LIST: &str = r#"List users on the fixed DRACOON target.

Purpose:
- Use this tool to list users available on the startup target.
- The server target is fixed at startup, so no target argument is required.

Arguments:
- `filter`: optional string filter passed through to the user list query.
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
  `{"filter": "alice", "all": true}`

Common failures:
- Using an invalid filter query.
- Missing stored auth for the fixed startup target.
"#;

pub const USERS_INFO: &str = r#"Fetch one user from the fixed DRACOON target.

Purpose:
- Use this tool to fetch one user by id or username on the startup target.
- The server target is fixed at startup, so no target argument is required.

Selector rules:
- Provide exactly one selector: `user_id` or `username`.

Arguments:
- `user_id`: optional unsigned integer. Preferred stable selector.
- `username`: optional string username selector.

Examples:
- Fetch by id:
  `{"user_id": 12345}`
- Fetch by username:
  `{"username": "alice"}`

Common failures:
- Providing both `user_id` and `username`.
- Providing neither `user_id` nor `username`.
- No user matches the provided selector.
- Missing stored auth for the fixed startup target.
"#;

pub const USERS_WHOAMI: &str = r#"Fetch the current authenticated user on the fixed DRACOON target.

Purpose:
- Use this tool to return the current authenticated user for the startup target.
- The server target is fixed at startup, so no target argument is required.

Arguments:
- This tool takes no arguments.

Examples:
- Fetch the current user:
  `{}`

Common failures:
- Missing stored auth for the fixed startup target.
- The authenticated username cannot be resolved in the user directory.
"#;

pub const USERS_CREATE: &str = r#"Create a user on the fixed DRACOON target.

Purpose:
- Use this tool to create one user on the startup target.
- The server target is fixed at startup, so no target argument is required.

Arguments:
- `first_name`: required string.
- `last_name`: required string.
- `email`: required string.
- `login`: optional string. Used for OIDC users and as the basic username override.
- `oidc_id`: optional unsigned integer. When provided, the new user is created with OIDC auth.
- `mfa_enforced`: optional boolean. Default `false`.
- `group_id`: optional unsigned integer for first group assignment.

Examples:
- Create a basic user:
  `{"first_name": "Alice", "last_name": "Admin", "email": "alice@example.com"}`
- Create an OIDC user:
  `{"first_name": "Alice", "last_name": "Admin", "email": "alice@example.com", "login": "alice", "oidc_id": 7}`

Common failures:
- Missing required user fields.
- Invalid OIDC or group assignment settings.
- Missing stored auth for the fixed startup target.
"#;

pub const USERS_GET_OIDC_IDPS: &str = r#"List OIDC identity providers on the fixed DRACOON target.

Purpose:
- Use this tool to list configured OpenID Connect identity providers on the startup target.
- The server target is fixed at startup, so no target argument is required.

Arguments:
- This tool takes no arguments.

Examples:
- Fetch configured OIDC identity providers:
  `{}`

Common failures:
- Missing stored auth for the fixed startup target.
- The startup target does not allow reading system authentication metadata.
"#;
