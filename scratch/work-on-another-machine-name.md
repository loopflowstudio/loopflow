# Sign a machine in as me — LOO-413

October 8 delivery update: Jack Heart accepted the reported Codex demo with
“sounds good. advance the task.” The saved `code` workflow advances through
`ship` (gate, then `pr land -c`). This proceeds beyond the prior
publication-and-review boundary. Gate findings and remaining CI coverage are recorded below. Unperformed provider,
later-refresh, reboot and installed-runtime acceptance remains unproved; the
accepted Codex demo does not invent those results.

October 8 update: Jack Heart authorized real-account verification for the
second-login demo, superseding the fixture-only restriction below for that check.
Codex immediate coexistence passed: original and second isolated login both
passed authenticated requests after browser approval. Jack then authorized
shipping the source binary to Mini. It is deployed with a separate data directory;
registration, real login transfer, authenticated reads on both machines,
account-selected remote execution and headless reconnect pass after Jack's
second browser approval. The configured attempt and evidence limits are recorded in
[machine sign-in demo](machine-sign-in-demo.md). Delivery now follows the update above.

Jack Heart authorized step 4 on 2026-10-07: implement fresh laptop logins installed
on an added machine, publish PR #1493, then stop for review. No landing,
real login, real credential read or real-account provider contact is authorized.
Jack Heart's latest October 7 steer records LOO-411 / PR #1489 merged and requests
sync to main, completion against `--machine`, and republication without landing.
Main `35e759aaf` is integrated through `715404677`; shared SSH, the install offer,
failure hints and the selector remain parent-owned. The parent's retired
catalog/provenance website assertions now check resident-login guidance.

The full remote-work design and dated decisions remain at
`8f270beaf3cf752756bb0aaf254cf9a37dbc368d:scratch/work-on-another-machine-name.md`.
That inherited document contained superseded rename/cache and installation prose;
LOO-411's accepted state is at `6b4769a974e52e29c076826cd6724e48b89dd6c4`.
The later Task placement, detached launch, per-Session relay, Desktop pane
restoration and waiting counts are separate Tasks. No stable old-peer interface,
shared resident service or automatic turn/Flow retry belongs here. No herdr/cmux
code, tests, configuration or copy is used.

## Implemented source behavior — reconciled 2026-10-07

`lf machine connect mini codex person@example.com` mints one login in a private
laptop directory using the saved browser choice, verifies identity, sends the
native file over SSH stdin and registers it on mini. Claude follows the same path.
`lf machine connect mini github` installs gh's selected token using its native
file-backed login. Linear mints a new OAuth grant in memory and installs the token
and refresh grant in the target SQLite store after comparing provider user IDs.
Neither provider account directories nor browser profiles are copied wholesale.

Jack Heart selected `lf --machine mini --account codex=person@ implement` on
October 7 (`e332b3bb-603b-4dc4-bee4-1e5b704d5721`). The global selector now
dispatches through the parent's transport entry; `lf ssh` is removed.
The launch path connects that account when missing and restricts the
remote invocation to the same login. Remote launches use isolated
account homes so native activation cannot require an unlocked Mac Keychain.
Unselected read commands do not mint logins. Missing logins require a foreground
terminal; headless callers get the explicit connect command rather than starting
background authorization. Existing target accounts remain, including unrelated
logins. A partial transfer never overwrites an existing native credential.

The existing machine identity and strict host-key checks precede transfer. The
receiving command is public (`machine credentials inspect/receive`); only stdin
carries secret bytes. Errors do not replay credential-consuming subprocess output.
There is no schema change.

The token encryption key is retained atomically in a private file. Existing
Keychain material must remain readable until captured; a locked/denied read must
never be interpreted as absent and replaced. Once the file exists, later SSH reads
avoid Keychain. New credential-free machines must be able to create their file
without an unlocked Keychain. Existing encrypted tokens must remain decryptable.

## Removed mechanisms

- SSH's ambient provider/GitHub/Linear token exports and Doppler `--secret` export.
- The detached-form string check and account broker/socket forwarding.
- Lent-account routing, remote catalog merging and injected Codex token login.
- The hidden account-lease probe, bearer-forwarding PM reader and optional SSH-only token resolver.
- Keychain key writes (which passed key bytes in argv) and truncate-in-place key files.
- Remaining lease-only selection wrappers, forwarding diagnostics and replay arguments.
- Route candidates' local/forwarded `source` field, per-candidate usage-window copies and duplicate store handles.
- Dynamic forwarded-secret environment expansion and broker-only fixture setup;
  retain ordinary Session credential and account-selection isolation.
- The `lf ssh` command, through LOO-411's global `--machine` selector cutover.

Managed account selection, local/shared behavior, native identity, provider-owned
refresh, historical captures, machine records and routes remain.

## Remaining work

- Continue PR #1493 through the saved `ship` edge after gate. Jack Heart's
  October 8 acceptance supersedes publication-only delivery. The Flow owns
  the following `pr land -c` operation; gate does not launch another Flow.
- Preserve the demonstrated Codex outcome and the unproved later independent
  refresh, Claude/Linear/GitHub, missing-account interactive launch, unrelated
  target-account live preservation, reboot/locked-Keychain and default-installed
  acceptance. No additional real login or installation ran during gate.
- CI owns the release-materialized Rust matrix and the Python suite under its
  supported isolation boundary. Local Python found two macOS sandbox limitations:
  redundant process-group cleanup returned EPERM after pipe EOF, and nested
  `sandbox-exec` could not apply its policy. The three direct headless cases
  passed; the 405 other Python cases passed with external networking denied.

## Gate findings — 2026-10-08

Machine status and version warnings now use the same build identity as
`lf --version`, retaining source suffixes. Equal-build SSH dispatch and status
are covered alongside the existing unequal-version/recovery cases.
The rendered security guide test still asserted removed broker behavior; it now
checks added-machine scope, stdin transfer, retained accounts and opt-in SSH
agent forwarding. The other ten rendered documentation checks passed.

Batch mode can inherit a foreground terminal. Missing login preparation now
honors `--batch` in both explicit connect and selected remote launch, while
already installed logins remain usable. A public CLI fixture gives each child
its own controlling foreground PTY to exercise that boundary.

The OAuth fixture was inspected: it joins its isolated tracing child with
`output()`, and its loopback server belongs to the child runtime. The exact
network-isolated test passed, then passed in the 520-case affected run with no
retained-output warning. The earlier leak's cause remains unknown; no speculative
cleanup change or retry was added. Release's bounded-cleanup lesson informed the
investigation, without importing its unrelated Swift diagnosis.

Review repairs: interrupted file-to-SQLite registration reuses the received login;
remote flags cannot relax the selected identity; credential-free and previously
encrypted machines differ when Keychain cannot be read. The obsolete lease owner,
lent routes and injected Codex-token login are deleted. Receiving uses `receive`
to preserve the established `lf install` shorthand. Release's operation-entry
lesson applies: SSH helper and receive-command proofs do not establish the four
required global-selector behaviors; the public-selector fixtures now cover them.
Review also exposed account flags after a skill bypassing local account preparation.
The remote dispatch reuses local argument reordering before selecting the resident
login, while `--` preserves literal prompt flags. Command and placement resolution
remain on the target. No additional product decision is needed.

Compression keeps one parsed remote command, verified fresh-login identity and
static Session credential scrub list. Lease-only wrappers, replay arguments and
broker fixtures are deleted. Routing candidates now retain their strain result
from one usage snapshot instead of copying every provider window into each
candidate and rescanning during sorting. A healthy named account still displaces
a strained active account. Route reports and their JSON fixture drop the constant
local/forwarded provenance; credential readiness borrows the existing home and
catalog directly. Candidate ordering borrows the machine store once; the health
writer is private and records native stream evidence. Linear PM access now has one
required-token path, deleting the SSH-only optional resolver and its conversion
helper. Its fixtures exercise that same path, retaining refresh-race, deletion and
rollback coverage. Replay still records no routing outcomes; local/shared behavior is retained.

Earlier selector, shell isolation and login/key proofs remain at
`d0134546fffbe68b2f8fe7d29c6e4c1f632617ca:scratch/work-on-another-machine-name.md`
and `7a3f263cb:scratch/work-on-another-machine-name.md`. The after-skill account
case failed before the argument-reordering repair and passed afterward. These
fixtures establish no real refresh-chain or installed acceptance.

Main-sync selector/login checks and earlier routing/rendered-guide evidence remain
at `d442386dbdc8d49cc85a64a34644e64f6483a3f1:scratch/work-on-another-machine-name.md`.

Checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `git diff --check`, `uv run python scripts/check_architecture.py` passed; with LF_/LOOPFLOW_ cleared, LF_BIN pinned and the absolute `scripts/test_network.py` Cargo runner, `cargo nextest run -p loopflow --lib --bin lf --test machine_commands --test machine_credentials --test auth_tests --test cli_discovery --test global_commands --test dto_fixtures --test documented_commands -E 'not binary(loopflow) | test(lf::) | test(provider_account::) | test(provider_auth::) | test(store::token_crypto::) | test(ops::pm::oauth_tests::) | test(engine::process::) | test(harness::codex::) | test(harness::conformance_tests::)' --no-fail-fast --build-jobs 4 --test-threads 4` passed 520/520; after batch repair, `cargo nextest run -p loopflow --lib --test machine_commands --test machine_credentials -E 'test(machine_credentials::) | binary(machine_commands) | binary(machine_credentials)' --no-fail-fast --build-jobs 4 --test-threads 4 --status-level fail` passed 22/22; `uv run --no-sync python scripts/test_network.py uv run --no-sync pytest python/tests/ -q` passed 405 with two sandbox-boundary failures, all three direct `test_desktop_performance.py -k 'cleanup_closes_descendant_pipes_even_after_leader_exit or checkout_observation_preserves_read_boundary_and_detects_changes'` cases passed; website `uv run python dev.py test -k test_docs` passed 10 plus the stale security test repaired and passed individually; release-materialized Rust and isolated Python completion remain CI-owned.
