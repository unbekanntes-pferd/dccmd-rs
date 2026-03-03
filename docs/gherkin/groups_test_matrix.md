# Groups Gherkin Coverage Matrix

This matrix maps each scenario in `groups.feature` to concrete automated tests.

| Scenario ID | Status | Test(s) |
|---|---|---|
| GR-001 | Covered | `test_list_groups_all_fetches_pages` ([src/app/groups/mod.rs](/home/oc/dev/dccmd-rs/src/app/groups/mod.rs)) |
| GR-002 | Covered | `test_create_group` ([src/app/groups/mod.rs](/home/oc/dev/dccmd-rs/src/app/groups/mod.rs)) |
| GR-003 | Covered | `test_delete_group_by_id`, `test_delete_group_by_name` ([src/app/groups/mod.rs](/home/oc/dev/dccmd-rs/src/app/groups/mod.rs)) |
| GR-004 | Covered | `test_delete_group_requires_name_or_id` ([src/app/groups/mod.rs](/home/oc/dev/dccmd-rs/src/app/groups/mod.rs)) |
| GR-005 | Covered | `test_list_group_users_for_single_group_name` ([src/app/groups/mod.rs](/home/oc/dev/dccmd-rs/src/app/groups/mod.rs)) |
| GR-006 | Covered | `test_add_group_user_resolves_names` ([src/app/groups/mod.rs](/home/oc/dev/dccmd-rs/src/app/groups/mod.rs)) |
| GR-007 | Covered | `test_add_group_user_requires_user_name_or_id` ([src/app/groups/mod.rs](/home/oc/dev/dccmd-rs/src/app/groups/mod.rs)) |
