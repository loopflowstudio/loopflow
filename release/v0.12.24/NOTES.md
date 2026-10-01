# v0.12.24

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.24 makes account access, Task execution, and unattended releases easier to inspect and recover. Authentication moves into one command family with retained usage observations, while branch builds keep private data and managed Tasks use their owning installation. Scheduled releases can hand failed checks to the landing repair supervisor and report failures back to cron.

## Inspect accounts and usage in one place

Login and account inspection now share `lf auth`. Browser choices are remembered independently for managed accounts and local services, and successful usage responses remain available for offline inspection.

- Use `lf auth status` or `lf auth status --details` to inspect account IDs, cached usage windows, their age and reset time, and remembered browser bindings. `lf auth status codex --verify` refreshes observations for the selected account set.
- Native browser callback completion and explicit hidden manual input replace Chrome page and clipboard scraping. New managed accounts are staged and checked for identity before registration.
- Status distinguishes managed credentials, local tokens, and uninspected forwarding, keeping missing credentials visible. Verification saves returned usage windows atomically while preserving routing and cooldown state.
- Account and route listings show IDs beside logins. Headless Runs record the account selected for each attempt before a provider Session ID exists.

## Keep development data separate from installed work

A branch build now seeds a private, source-specific data directory once, preserving subsequent private writes and clearing inherited execution authority. Managed Task operations route to the selected installation before preparation, checkpoints, or worker claims, keeping its executable and database together.

- `lf task run` starts or continues saved work; `lf task restart` replaces its invocation. Preparation is now `lf task checkout`, which can restore missing checkouts from retained history and work from dirty checkouts.
- Managed operations return installed Task state and consume installed review readiness without transferring branch-only identities or feedback.
- Opening a Wave chat connects it automatically. Separate Wave lifecycle and Task/Wave enablement controls are removed from the CLI, app, and wire models.
- Task execution cannot change installations. Incompatible-database errors retain migration evidence and recommend recovery pairs only after checking their artifacts and exact database compatibility.

## Recover release checks and report failures

Scheduled releases now use the existing landing repair supervisor when required checks fail. The release path retains version-metadata rebuilding when main advances and preserves blocked repair checkouts for recovery.

- Cron receives the release operation's actual exit status. The scheduled release target resolves to the deterministic operation Flow before the same-named skill.
- GitHub's temporary no-checks response after a push remains pending instead of being treated as a completed check result.
- The blocking Swift fixture join reproduced locally is replaced with bounded asynchronous exit observation, addressing the cleanup defect investigated after the previous release's test hang. The original hosted job retained no stack, so that diagnosis remains limited to the local reproduction.

## Operational notes

- **Command migration:** six `lf auth` subcommands replace the old account/profile/access commands and top-level routing commands. Readers move to `lf catalog`, `lf wave list`, `lf wave status`, `lf wave probe`, and `lf pr checks`; removed commands have no compatibility aliases. Update scripts for these changes and the Task commands above.
- The auth migration preserves populated account, profile, and route history. Published Rust API replacements are documented in `docs/subscriptions.md` under Rust API migration.
- Branch and installed databases do not synchronize, so branch status can differ from state returned by a managed operation. Private databases do not isolate provider mutations or shared checkout edits. Remote placement remains supported; remote chat transport is not added to the app.
- Recorded checks include 338 Swift tests, simulated failed-check repair before tagging, and all four disposable Linux installation proofs. Live installed-worker acceptance and live release publication remain unproven in the supplied evidence.
- Browser login without pasted codes, first-time managed connection, remembered Linear profile targeting, and real Claude/Codex usage windows also remain unproven. The configured Claude probe returned `invalid_grant`; its decoder fixture is synthetic. A copied-Home demo retained a pre-migration detailed-status failure and a stored-route/absent-credential disagreement. Cross-account Session continuation, native credential refresh coordination, and remaining-headroom ranking are outside this release's auth scope.

## Small changes

- Cold installation-proof image downloads have a network deadline separate from container creation.
- CI adds a required disposable Linux installation job covering continuation, review completion, store protection, and recovery.