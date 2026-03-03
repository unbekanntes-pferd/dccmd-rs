# Transfer Gherkin Coverage Matrix

This matrix maps each scenario in `transfer.feature` to concrete automated tests.

| Scenario ID | Status | Test(s) |
|---|---|---|
| TR-001 | Covered | `test_execute_dispatches_transfer_to_app_command` ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)) |
| TR-002 | Covered | `test_transfer_delegates_to_platform` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| TR-003 | Covered | `test_transfer_propagates_platform_error` ([src/app/mod.rs](/home/oc/dev/dccmd-rs/src/app/mod.rs)) |
| TR-004 | Missing | _No dedicated unit test yet in transfer service for encrypted target with `--share`._ |
| TR-005 | Missing | _No dedicated unit test yet in transfer service for source/target path resolution failures._ |
