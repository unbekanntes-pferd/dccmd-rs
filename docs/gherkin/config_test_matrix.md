# Config Gherkin Coverage Matrix

This matrix maps each scenario in `config.feature` to concrete automated tests.

| Scenario ID | Status | Test(s) |
|---|---|---|
| CFG-001 | Covered | `test_normalize_base_url_strips_path_and_scheme` ([src/app/config/service.rs](/home/oc/dev/dccmd-rs/src/app/config/service.rs)) |
| CFG-002 | Covered | `test_remove_refresh_token_uses_normalized_base_url` ([src/app/config/service.rs](/home/oc/dev/dccmd-rs/src/app/config/service.rs)) |
| CFG-003 | Covered | `test_has_encryption_secret_returns_false_when_missing`, `test_has_encryption_secret_returns_true_when_present`, `test_remove_encryption_secret_uses_crypto_account` ([src/app/config/service.rs](/home/oc/dev/dccmd-rs/src/app/config/service.rs)) |
| CFG-004 | Covered | `test_execute_dispatches_config_to_app_command` ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)) |
| CFG-005 | Partial | `test_execute_propagates_app_executor_error` validates propagation at runner level ([src/cli/runner.rs](/home/oc/dev/dccmd-rs/src/cli/runner.rs)); _no dedicated config system-info app service unit test yet._ |
