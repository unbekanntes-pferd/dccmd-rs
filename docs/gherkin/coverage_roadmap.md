# Gherkin Coverage Rollout Order

## Goal
Complete decision and behavior coverage across all CLI command domains with a unit-first test pyramid and explicit scenario-to-test traceability.

## Ordered execution
1. `nodes.feature` / `nodes_test_matrix.md`
   - Keep as the baseline for node primitives.
   - Add and maintain download/upload concurrency scenarios first because transfer risk is highest in I/O paths.
2. `upload.feature` / `upload_test_matrix.md`
   - Lock recursive upload, public share constraints, overwrite/share semantics, and concurrency limits.
3. `transfer.feature` / `transfer_test_matrix.md`
   - Cover stream handoff, error propagation, and encrypted/share guardrails.
4. `users.feature` / `users_test_matrix.md`
   - Cover selector validation, pagination, auth switching, and MFA enforcement.
5. `groups.feature` / `groups_test_matrix.md`
   - Cover user resolution and group/user mutation constraints.
6. `reports.feature` / `reports_test_matrix.md`
   - Cover pagination, API-version guards, filtering modes, and option parsing.
7. `config.feature` / `config_test_matrix.md`
   - Cover URL normalization, secret/token storage operations, and command dispatch.
8. `cli.feature` / `cli_test_matrix.md`
   - Lock top-level dispatch and exit-code contracts.

## CI policy
- Default CI (`cargo test`): fast deterministic unit/integration tests only.
- Heavy concurrency tests: deterministic and bounded; avoid network.
- Live DRACOON E2E: gated or nightly only.

## Matrix status vocabulary
- `Covered`: scenario is directly asserted by one or more tests.
- `Partial`: some behavior is asserted, but one or more acceptance aspects are not.
- `Missing`: no direct automated assertion exists yet.
- `Out-of-scope`: intentionally not covered in unit/integration (e.g., external service/network behavior).
