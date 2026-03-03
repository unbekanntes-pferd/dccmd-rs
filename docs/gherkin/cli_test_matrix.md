# CLI Gherkin Coverage Matrix

This matrix maps each scenario in `cli.feature` to concrete automated tests.

| Scenario ID | Status | Test(s) |
|---|---|---|
| CLI-001 | Covered | `test_execute_dispatches_ls_to_app_command` ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)) |
| CLI-002 | Covered | `test_execute_dispatches_mkroom_alias_to_mkdir_app_command` ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)) |
| CLI-003 | Covered | `test_execute_dispatches_transfer_to_app_command` ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)) |
| CLI-004 | Covered | `test_execute_dispatches_config_to_app_command` ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)) |
| CLI-005 | Covered | `test_execute_propagates_app_executor_error` ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)) |
| CLI-006 | Covered | `test_execute_converts_error_outcome_to_command_failed` ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)) |
