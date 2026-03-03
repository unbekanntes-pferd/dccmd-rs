# Reports Gherkin Coverage Matrix

This matrix maps each scenario in `reports.feature` to concrete automated tests.

| Scenario ID | Status | Test(s) |
|---|---|---|
| RP-001 | Covered | `test_check_dracoon_api_version_rejects_v5` ([src/app/reports/mod.rs](/home/oc/dev/dccmd-rs/src/app/reports/mod.rs)) |
| RP-002 | Covered | `test_get_events_collects_all_pages` ([src/app/reports/mod.rs](/home/oc/dev/dccmd-rs/src/app/reports/mod.rs)) |
| RP-003 | Covered | `test_event_options_parses_start_and_end_dates`, `test_event_options_parses_status`, `test_event_options_new_params_with_offset` ([src/app/reports/mod.rs](/home/oc/dev/dccmd-rs/src/app/reports/mod.rs)) |
| RP-004 | Covered | `test_get_permissions_uses_filter_without_listing_users` ([src/app/reports/mod.rs](/home/oc/dev/dccmd-rs/src/app/reports/mod.rs)) |
| RP-005 | Covered | `test_get_permissions_collects_by_each_user_without_filter` ([src/app/reports/mod.rs](/home/oc/dev/dccmd-rs/src/app/reports/mod.rs)) |
| RP-006 | Covered | `test_get_event_operations_delegates_to_api` ([src/app/reports/mod.rs](/home/oc/dev/dccmd-rs/src/app/reports/mod.rs)) |
