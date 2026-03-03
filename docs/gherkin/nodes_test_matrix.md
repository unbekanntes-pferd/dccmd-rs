# Nodes Gherkin Coverage Matrix

This matrix maps each Gherkin scenario in `nodes.feature` to concrete unit tests.

| Scenario ID | Status | Test(s) |
|---|---|---|
| LS-001 | Covered | `test_list_nodes_uses_get_nodes_for_non_search` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)), `test_ls_collects_info_outcome` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| LS-002 | Covered | `test_list_nodes_uses_search_for_wildcard` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| LS-003 | Covered | `test_list_nodes_all_fetches_next_page` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| CP-001 | Covered | `test_copy_nodes_single_source_node` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)), `test_cp_writes_success_outcome` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| CP-002 | Covered | `test_copy_nodes_search_source` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| CP-003 | Covered | `test_copy_nodes_errors_when_source_missing` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| CP-004 | Covered | `test_copy_nodes_errors_when_target_missing` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| RM-001 | Covered | `test_prepare_delete_requires_recursive_for_search_query` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)), `test_rm_invalid_search_requires_recursive_writes_error` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| RM-002 | Covered | `test_prepare_delete_requires_recursive_for_container` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)), `test_rm_container_requires_recursive_writes_error` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| RM-003 | Covered | `test_prepare_delete_search_filters_out_rooms` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)), `test_rm_search_confirmed_deletes_batch` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| RM-004 | Covered | `test_rm_search_cancelled_skips_batch_delete` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| RM-005 | Covered | `test_rm_room_confirmed_deletes_single_node` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| RM-006 | Covered | `test_rm_room_cancelled_writes_error` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| MKDIR-001 | Covered | `test_mkdir_default_folder_writes_success_without_warning` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)), `test_create_folder_success` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| MKDIR-002 | Covered | `test_mkdir_alias_writes_deprecation_warning` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| MKDIR-003 | Covered | `test_create_folder_rejects_room_only_options` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| MKDIR-004 | Covered | `test_create_room_inherit_defaults_to_true_without_admin_users` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| MKDIR-005 | Covered | `test_create_room_inherit_follows_flag_with_admin_users` ([src/app/nodes/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/mod.rs)) |
| DL-001 | Covered | `test_download_delegates_to_platform` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| DL-002 | Covered | `test_search_query_files_returns_files_only` ([src/app/nodes/download/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/mod.rs)), `test_get_files_filters_non_file_nodes` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |
| DL-003 | Covered | `test_search_query_files_fetches_next_page` ([src/app/nodes/download/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/mod.rs)), `test_get_files_fetches_next_page` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |
| DL-004 | Covered | `test_get_containers_excludes_rooms_when_include_rooms_is_false` ([src/app/nodes/download/containers.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/containers.rs)), `test_filter_files_in_sub_rooms_excludes_nested_room_files` ([src/app/nodes/download/containers.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/containers.rs)) |
| DL-005 | Covered | `test_plan_container_download_unix` ([src/app/nodes/download/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/mod.rs)), `test_plan_container_download_windows` ([src/app/nodes/download/mod.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/mod.rs)), `test_join_paths_windows` ([src/app/nodes/filesystem.rs](/home/oc/dev/dccmd-rs/src/app/nodes/filesystem.rs)) |
| DL-006 | Covered | `test_download_public_file_rejects_recursive` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |
| DL-007 | Covered | `test_parse_public_download_access_key_rejects_empty_tail` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |
| DL-008 | Covered | `test_download_files_errors_when_target_missing` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |
| DL-009 | Covered | `test_download_files_success_writes_all_targets` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |
| DL-010 | Covered | `test_download_files_aggregates_partial_download_error` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)), `test_download_partial_failure_writes_warning_and_details` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| DL-011 | Covered | `test_download_files_aggregates_all_failed_downloads` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)), `test_download_all_failed_writes_error_and_truncated_details` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| DL-012 | Covered | `test_filter_files_in_sub_rooms_fetches_additional_pages` ([src/app/nodes/download/containers.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/containers.rs)) |
| DL-CONC-001 | Covered | `test_effective_velocity_applies_defaults_and_clamps` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |
| DL-CONC-002 | Covered | `test_download_files_high_concurrency_respects_velocity_cap` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |
| DL-CONC-003 | Covered | `test_download_files_low_velocity_limits_parallelism` ([src/app/nodes/download/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/download/files.rs)) |

## Remaining risk notes
- Public-share happy-path networking is covered via mocked integration tests:
  `test_download_public_file_happy_path_with_http_mock` and
  `test_upload_public_file_happy_path_with_http_mock`.
- Residual risk is limited to real-server behavior outside mocks (e.g., live environment differences and end-to-end auth/TLS).
