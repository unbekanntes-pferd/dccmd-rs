# Upload Gherkin Coverage Matrix

This matrix maps each scenario in `upload.feature` to concrete automated tests.

| Scenario ID | Status | Test(s) |
|---|---|---|
| UP-001 | Missing | _No dedicated automated upload service success test yet._ |
| UP-002 | Missing | _No dedicated automated upload service recursive-flag validation test yet._ |
| UP-003 | Missing | _No dedicated automated upload service public-share directory rejection test yet._ |
| UP-004 | Covered | `test_list_directories_uses_mock_fs_tree`, `test_list_files_uses_mock_fs_tree`, `test_mac_path_normalization` ([src/app/nodes/upload/folders.rs](/home/oc/dev/dccmd-rs/src/app/nodes/upload/folders.rs)) |
| UP-CONC-001 | Covered | `test_effective_velocity_applies_defaults_and_clamps` ([src/app/nodes/upload/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/upload/files.rs)) |
| UP-CONC-002 | Covered | `test_concurrent_requests_for_velocity_scales_by_multiplier` ([src/app/nodes/upload/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/upload/files.rs)) |
| UP-CONC-003 | Covered | `test_high_concurrency_bounded_execution_respects_cap` ([src/app/nodes/upload/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/upload/files.rs)) |
| UP-CONC-004 | Covered | `test_low_velocity_bounded_execution_respects_cap` ([src/app/nodes/upload/files.rs](/home/oc/dev/dccmd-rs/src/app/nodes/upload/files.rs)) |
