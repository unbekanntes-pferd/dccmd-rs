# Users Gherkin Coverage Matrix

This matrix maps each scenario in `users.feature` to concrete automated tests.

| Scenario ID | Status | Test(s) |
|---|---|---|
| US-001 | Covered | `test_list_users_collects_all_pages` ([src/app/users/mod.rs](/home/oc/dev/dccmd-rs/src/app/users/mod.rs)) |
| US-002 | Covered | `test_delete_user_by_name_resolves_id_and_deletes` ([src/app/users/mod.rs](/home/oc/dev/dccmd-rs/src/app/users/mod.rs)) |
| US-003 | Covered | `test_delete_user_requires_identifier` ([src/app/users/mod.rs](/home/oc/dev/dccmd-rs/src/app/users/mod.rs)) |
| US-004 | Covered | `test_resolve_invite_room_id_for_room`, `test_resolve_invite_room_id_for_folder`, `test_invite_user_delegates_to_api` ([src/app/users/mod.rs](/home/oc/dev/dccmd-rs/src/app/users/mod.rs)) |
| US-005 | Covered | `test_read_user_imports_from_csv_parses_rows`, `test_import_users_counts_success_and_failures` ([src/app/users/mod.rs](/home/oc/dev/dccmd-rs/src/app/users/mod.rs)) |
| US-006 | Covered | `test_switch_auth_updates_only_matching_users` ([src/app/users/mod.rs](/home/oc/dev/dccmd-rs/src/app/users/mod.rs)) |
| US-007 | Covered | `test_enforce_mfa_with_group_id_and_auth_filter`, `test_enforce_mfa_counts_failures` ([src/app/users/mod.rs](/home/oc/dev/dccmd-rs/src/app/users/mod.rs)) |
| US-008 | Covered | `test_enforce_mfa_requires_selector` ([src/app/users/mod.rs](/home/oc/dev/dccmd-rs/src/app/users/mod.rs)) |
