# Sign a machine in as me — LOO-413

Jack Heart authorized step 4 on 2026-10-07: implement fresh laptop logins installed
on an added machine, publish the stacked PR, then stop for review. No landing,
real login, real credential read or real-account provider contact is authorized.
LOO-411 / PR #1489 owns the parent machine records. Its later shared connection,
remote-install offer, failure hints and global selector are integrated from parent
`72f06221183c771ddd43eaabd8f968417a82c571` (October 7 sync).

The full remote-work design and dated decisions remain at
`8f270beaf3cf752756bb0aaf254cf9a37dbc368d:scratch/work-on-another-machine-name.md`.
That inherited document contained superseded rename/cache and installation prose;
LOO-411's accepted state is at `6b4769a974e52e29c076826cd6724e48b89dd6c4`.
The later Task placement, detached launch, per-Session relay, Desktop pane
restoration and waiting counts are separate Tasks. No stable old-peer interface,
shared resident service or automatic turn/Flow retry belongs here. No herdr/cmux
code, tests, configuration or copy is used.

## Outcome and implementation

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

## Delete — do not maintain

- SSH's ambient provider/GitHub/Linear token exports and Doppler `--secret` export.
- The detached-form string check and account broker/socket forwarding.
- Lent-account routing, remote catalog merging and injected Codex token login.
- The hidden account-lease probe and the obsolete bearer-forwarding PM reader.
- Keychain key writes (which passed key bytes in argv) and truncate-in-place key files.
- Remaining lease-only selection wrappers, forwarding diagnostics and replay arguments.
- Dynamic forwarded-secret environment expansion and broker-only fixture setup;
  retain ordinary Session credential and account-selection isolation.
- The `lf ssh` command, through LOO-411's global `--machine` selector cutover.

Preserve managed account selection, local/shared account behavior, native identity,
provider-owned refresh, historical captures, current machine records and routes.

## Remaining work

- Publish the stacked PR and stop for Jack Heart's review. No landing.
- Gate/CI owns the full affected suite. The focused source checks cover account
  routing, native receipt/registration, byte preservation, command discovery and
  token-key handling. No installed store or real credential was used.
- Hand verification, first: a second login for the same account leaves the first
  signed in. Then independent Claude/Codex refresh chains; independent Linear
  grants refreshed on different days; copied gh token usable on both machines;
  Mac after reboot with a locked Keychain; at most one browser approval per account
  and no remote terminal. Fixtures cannot establish these results. Real-account
  verification requires separate authorization under Jack Heart's fixture-only scope.

Review repairs: interrupted file-to-SQLite registration reuses the received login;
remote flags cannot relax the selected identity; credential-free and previously
encrypted machines differ when Keychain cannot be read. The obsolete lease owner,
lent routes and injected Codex-token login are deleted. Receiving uses `receive`
to preserve the established `lf install` shorthand. Release's operation-entry
lesson applies: SSH helper and receive-command proofs do not establish the four
required global-selector behaviors. No additional product decision is needed.

Compression reuses the verified fresh-login identity and parses the remote CLI
once. Account selection no longer wraps each resolved provider/account pair;
the replay API no longer accepts a Session ID used only by the deleted broker.
Literal account/home flags after `--` remain prompt text. Provider selection's
ambiguity and duplicate checks are retained on the surviving local path.

The follow-up compression removes broker socket/token fixtures and the retired
lease/forwarded-secret environment reader and scrub lists. Session startup uses
one static list for current credentials and account choices. A shell execution
test proves those values are absent in the child; local account reporting still
checks cached/live evidence and successful/failed Process outcomes without a
provider executable. No old-peer compatibility path is retained.

Checks: network-isolated `cargo nextest run -p loopflow --test machine_commands --test cli_discovery -E 'binary(machine_commands) | test(remote_selection_preserves_command_arguments_and_literal_boundaries)' --no-fail-fast` passed 12/13; removing origin-side shorthand resolution repaired dispatch, then `cargo nextest run -p loopflow --test machine_commands -E 'test(machine_selector_dispatches_before_local_help_and_placement) | test(selected_machine_login_stays_restricted_and_missing_login_stops_headless)' --no-fail-fast` passed 2/2 with inherited LF_/LOOPFLOW_ authority cleared and LF_BIN pinned to this checkout; `cargo fmt --all` and `git diff --check` passed. Global-selector fixtures cover selected-account restriction, literal prompt flags, headless missing-login recovery and retired-command rejection. Earlier focused evidence is retained in pre-sync history; gate/CI owns broader proof.
