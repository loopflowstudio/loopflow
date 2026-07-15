# Profile routing QA

## Blocking issues

None remain.

The review found and fixed four account-isolation failures before the final gate:

- Routed provider children inherited `LF_FORWARDED_PROFILE_BUNDLE`, exposing credentials for every forwarded account instead of only the selected account. `ProviderAccountRoute` now strips the bundle before adding the chosen credential.
- Disconnecting a managed Claude account called the ambient CLI logout and could remove the operator's global Keychain login. Managed homes now delete only their own native auth state.
- A managed Claude login with no previous ambient Keychain item left its new credential behind. `ClaudeKeychainGuard` now restores both prior-value and prior-absence states, including a drop-time retry.
- Provider OAuth could finish under a different Google identity than the chosen Chrome profile. Connect and import now reject that mismatch before persisting the account.

Profile creation also validates the Chrome identity before writing the profile, and the auth-help assertion and README examples now match the shipped CLI.

## Polish items

- The existing development database under `~/.lf-dev/worktrees/loopflow-auth-compat-538da878f402` was created by the pre-rebase migration numbering. Back it up and repair its ledger before using this worktree's debug CLI against it. The production database is unaffected.
- The Context Lab worktree independently carries the same `0.11.008` and `0.11.009` migration files. Preserve a single copy when that branch is integrated after profile routing.
- Hosted UI behavior was not run because this change has no UI surface. The required app and UI-test runners compiled successfully.

## Test results

- `uv run python scripts/test.py --all`: 6 suites passed in 87 seconds.
- Rust: 1,356 passed, 2 skipped; formatting and clippy with warnings denied passed.
- Python: 59 passed.
- Website: 59 passed, 3 skipped.
- Swift: 108 passed; multiplatform boundary checks passed.
- E2E smoke passed.
- Loopflow macOS app and UI-test runners compiled.
- `uv run python scripts/demo_profile_routing.py`: passed against an isolated temporary home and reproduced the intended three-profile route.
- Migration tests passed, and a backup copy of the live database upgraded through `0.11.011_provider_account_lifecycle` without modifying the live database.

## Review verdict

The public model maps directly to the real account structure: a profile is one Google/Chrome identity, each provider maps independently, and repository routes order profiles rather than credential blobs. Selected credentials are isolated at the process boundary, managed Claude state no longer mutates ambient auth on disconnect, and identity mismatches fail before persistence. The branch is ready for a backed-up local install and live account setup.
