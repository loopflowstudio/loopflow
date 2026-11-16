# Remove subscription management

LOO-913 removes subscription login management from native launches. Claude and
Codex own authentication; lf owns Sessions, work and model/harness continuation.
The delivery is one reviewable PR, without merging or landing.

## Direction

Comment 6b066cf0-b498-488e-8332-9f6d87583df4, by Session
`session_5d8c1740f15643fea778d99c8553ab41` for Jack Heart, says:

> Finish removal in the existing checkout, retaining uncommitted changes and scratch/subscription-removal.md. Native auth owns logins; delete account selection/activation/rotation and management surfaces, retaining history and stored credentials without destructive migrations. Both headless cases honor configured model/harness fallback; both interactive cases require explicit retry or move. Bulk rollover and richer recovery stay in follow-up 3845de. Publish a reviewable PR. Do not merge or land this Task.

This supersedes the earlier open choice about Task-owned interactive retry.
Bare and Task-owned interactive Sessions keep the refusal visible and send no
automatic follow-up input. Bare headless and Task workers may move through the
configured model/harness order after a classified refusal. Exhaustion stops with
the provider error and a useful next step; there is no account-reset wait loop.
An external login change or purchase may make a person's later retry succeed.

## Experience and demo

An isolated fixture gives the native home login A, while lf's saved catalog has
login B preferred and every saved account cooling. `lf -b <fixture-skill>` starts
using A without selecting, refreshing or installing B. A provider-authored quota
refusal moves a configured headless Claude run to Codex and retains the Session's
prior history. The same refusal in an interactive launch leaves that provider
and thread open for a person's next action, with exactly one input dispatch.
Task membership does not change either policy.

Resume an older conversation stored under a managed home: the same native thread
opens using the native login, without copying credentials. Plain native clients
and lf find the shared transcript. Existing receipts and account attribution
remain readable; new runs do not claim to know the active subscription identity.

## Removal map at base `9cafc410`

- `agent/mod.rs::_pin_provider_account_id`, `resolve_account_route_blocking`,
  `_run_harness_once` and `run_interactive` select before launch, pin account
  authority into capture and activate native credentials. `AgentAttempt`,
  `CapacityWait`, `MAX_ACCOUNT_FAILOVERS` and `_run_with_transient_retries` combine
  account rotation/reset waits with useful model/harness continuation. Existing
  edits change `_fallthrough_rungs` to headless (`process.auto`), but account
  authority and reset machinery remain. Delete the account branch rather than
  changing its ranking.
- `harness/{claude,codex}.rs::start` resolves again; subprocess startup activates
  the route. Their readers gate some refusal events on `account_route.is_some()`
  (`fails_on_limit` in Claude). Removing the route without changing those gates
  loses headless fallback. Emit failure from an actual rejected current turn,
  independently of catalog presence. A quota snapshot, exhausted window or
  assistant prose alone must not terminate a successful turn.
- `harness/fact_writer.rs::FactWriter::write` couples Session receipts to
  `route.pin_session` and `route.record_rate_limit`; decoders import
  `provider_account::RateLimitSignal`. Move surviving passive signal types beside
  harness facts and retain normalized provider errors/usage, with no account
  writes, credential validation or provider polling.
- `agent/session.rs::resolve_session_config`, `lf/commands/{util,replay}.rs` and
  `ops/human_session.rs` carry account pins through resume, replay and open.
  `human_session::open` can stop Codex or switch Claude on an account selector.
  Delete that behavior, retaining explicit model/harness moves and native thread
  ownership. Historical `account_id` is metadata, never replay authority.
- `provider_account.rs::ProviderAccountRoute::adopt` also copies old transcripts.
  Deleting it blindly strands conversations. Move transcript-only adoption into
  `transcript/`, using `conversation_home` and `copy_between`; prefer an existing
  native transcript and retain newer copies. Update the conversation's home
  observation after adoption through the existing store mechanism. Never copy
  auth files or change HOME to launch a saved login. If the transcript is missing,
  report that thread's missing history instead of silently opening an empty one.
- `transcript/mod.rs` imports `activation::native_home`. Move native-home lookup
  out of activation into the surviving provider/transcript module; keep honoring
  caller `CODEX_HOME` / `CLAUDE_CONFIG_DIR` and API-key environment. A sandbox
  execution boundary must work with native auth; remove the managed-account
  prerequisite in `resolve_account_route_blocking` while retaining its filesystem
  and network restrictions.
- `ops/human_session.rs` uses `LaunchCapacity` for listings, filters and move/open.
  `lf/commands/{screens,waves}.rs` also reads it. A saved catalog cannot establish
  a native login's present capacity after removal. Delete catalog-derived blocked
  state and account selectors across CLI, wire and Swift; retain historical
  account labels and actual recorded provider refusals. Do not keep a false
  capacity badge or replace it with a constant success badge.
- `lf/commands/ssh.rs` calls `machine_credentials::prepare_launch` before remote
  execution; rental preparation installs Claude accounts and requires unattended
  logins. Remove Claude/Codex credential transfer and automatic provisioning.
  The destination native client owns its credentials and displays its own missing
  auth error. Keep Machine routing, repository setup, and unrelated GitHub/Linear
  credentials and OpenCode behavior.
- `lf/commands/account.rs` and `provider_auth/` also implement GitHub, Linear,
  Asana, Notion, Doppler and OpenCode authentication. Those are not subscription
  rotation. Retain their connect/disconnect, browser venue and credential flows;
  remove agent subscription management instead of deleting the shared namespace
  and breaking planning or repository access.
- `docs/subscriptions.md` and `docs/config.md` currently promise routing, reset
  waits and account-selected remote launch. Change these with the code. Server
  `subscription*` modules are read/watch subscriptions, not provider billing;
  they survive.

## Target and source of truth

Native provider configuration, env and login own launch authentication. No lf
account catalog, login availability or saved route participates in admission.
Configured `default_agent` / `agents`, skill overrides and explicit pins own
headless model/harness choice. An explicit agent remains pinned according to
existing configuration semantics. `ProcessConfig::auto` owns retry policy;
Task association owns reporting, never permission to replay input.

Sessions, input/turn receipts, native thread IDs and provider-authored usage stay
in their existing records. Runtime transitions keep earlier outcomes and use the
existing transcript seeding when changing harness. New account identity is
unknown/null unless reported by the native provider; no inference from stale
managed account rows. Retain required nullable DTO keys with no decode defaults.

All existing account/token/limit/switch/session-account tables, migrations,
encrypted tokens and on-disk homes stay intact. Retain read helpers needed for
historical labels or transcript lookup. Remove unused runtime account writers
and ranking queries, without adding a migration or exporting credentials in this
Task. Remove account preference interpretation/editing from the typed catalog;
do not rewrite or erase existing personal configuration. Existing released
configuration remains readable; obsolete saved fields must be inert on launch.
Any necessary released-file compatibility parsing gets a named removal Task,
not a second routing implementation.

## Delete — do not maintain

1. `ContextMoment::{Limit,RecordLimit}`, StopFailure settings, detached
   `os_process::detached_context_block`, limit recorder and their exclusive tests.
   The existing uncommitted deletion is retained; finish its dangling imports,
   constants and parser expectations. Keep start/tool/stop hooks and recording-gap
   behavior for surviving hooks.
2. `ProviderAccountRoute`, selection/resolution/activation/holds and Codex access
   repair in `provider_account.rs` and `provider_account/`; launch account fields,
   pinning helpers, credential locks, same-provider rotation, account wait state
   and prompts. Move only surviving native-home, transcript and passive-error
   behavior before deleting these modules. Delete exclusive ranking, refresh,
   handoff, switch and wait tests; retain model fallback and actual refusal tests.
3. Claude/Codex account OAuth capture/import/refresh, subscription quota polling
   and reset redemption (`subscription.rs`, agent-only `provider_auth/` pieces,
   `lf/commands/account_daemon_tests.rs`). Do not remove provider-native stream
   usage receipts, generic token storage, PM auth or the Codex app-server itself.
4. `--account`, `--only-account`, account-selection env forwarding and root/Graph
   propagation; runtime account pins in resume/replay/open and account movement
   actions. `LaunchCapacity`, account-backed `--blocked` / login `--on` selectors
   and their exclusive fixtures disappear. Harness/model `--on` filtering stays.
   Historical departure/account observations continue to decode.
5. Subscription `account next/use/pause/unpause/set/redeem-reset`, managed-login
   connect/disconnect/status and subscription-specific config declarations.
   Reduce `account.rs`, `account_status.rs`, `profile.rs`, `engine/account.rs` to
   surviving service-auth behavior; remove empty modules. Remove corresponding
   `read.rs`/`write.rs` variants, schema examples and CLI parser cases. Preserve
   unrelated service auth and Chrome profile selection where still used.
6. Claude/Codex `machine_credentials.rs` credential variants, remote send/receive,
   rental refresh/identity machinery and launch preparation, plus their CLI and
   tests. Keep surviving service credential transfer and remote routing.
7. Swift `RegistryQuery.swift` account operations/selectors and matching
   `ServerProtocolTests`, plus any views consuming removed fields. Remove stale
   selection env allowlisting in `LoopflowMac/Services/ProcessEnvironment.swift`.
   Regenerate server schema and update shared DTO fixtures on both sides.
8. Retire `claude_shared_home.py`, Codex's `_shared_provider_home_contract` and
   `test_auth_cron_evidence.py`: they exclusively exercise removed switching and
   admission. Preserve native launch/adoption coverage in the surviving Rust
   fixtures; remove the managed-login prerequisite in `bootstrap-cron-host.sh`.
9. Rewrite routing promises in `docs/subscriptions.md`, config, remote/rental
   docs, harness and tests READMEs and architecture references. Retire website
   `test_docs_subscriptions_page_owns_account_selection`; replace it with a check
   of the native-auth/retry contract. No private extraction or policy discussion
   belongs in product docs.

## Alternatives and failure modes

Keeping a disabled selector or account-service adapter leaves credential authority
inside lf and retains most maintenance; reject both. A proxy adds an inference
transport and needs independent compatibility validation; outside this removal.
The earlier comparison and policy research are background, not another design.
No new external-provider API is required for this cut; no live inference probe is
needed to choose it.

Success means a person can change a native login and the next lf launch simply
uses it, while unattended work still follows configured provider fallbacks.
The serious failure is a hidden writer restoring a retired login, or a removal
that loses threads, usage, PM auth or headless recovery. End-to-end stand-ins,
not only parser assertions, must cover those cases. Do not ship stubs, hidden
commands, dual selectors, auto-resends for interactive runs, quota pollers, or an
account prerequisite imposed by execution isolation.

## Internal slices and remaining work

### Concept review — 2026-11-15

Native login, Session history and headless continuation remain separate concepts.
Task membership does not grant permission to resend interactive input. The normal
launch, refusal/recovery matrix and transcript-only adoption fit that model; no
new product choice is needed.

Historical attribution now reads as history: `lf session list` renders
retained `session.account` as “historical account: {account}”, and the Rust
`SessionSummary::account` and Swift `SessionRecord.account` doc comments call it
historical attribution, never the current native login. Stored value and wire
key are unchanged. No test asserts the rendered label yet; gate or qa checks it
on an old attributed Session after native resume and on a new Session with no
attribution. The concept review page is `scratch/concept-review.html`.

One architectural cut ships as one PR; these are implementation order, not
separate Tasks or compatibility stages.

The runtime, CLI/server/Swift surfaces, remote brokerage and subscription
polling cuts are implemented. Historical storage has no destructive migration;
raw fixture insertion is test-only. Native stderr failures now remain visible
when a provider exits before emitting its first structured event. Detached
launches inherit native provider homes and API-key environment.

The native-auth implementation, its focused acceptance and the attribution
wording are complete. Gate owns
the full changed-path suites and final review; publication follows in the caller's
delivery step. No merge or landing belongs to this implementation step.

Follow-up 3845de owns bulk rollover and richer recovery: preview, busy-turn
handling, retained drafts/approvals, same-thread restart, correlated rejected-turn
retry and uncertain delivery. LOO-811 / PR #1862 is an unmerged reference; do not
merge or operate it. Task-interactive automatic recovery is outside this cut.

## Acceptance and checks

Use isolated HOME/LF_HOME, provider stand-ins and no paid inference, refresh,
production account changes or live credential export. The stand-in records inputs,
thread IDs and credential markers privately; assertions never print secrets.

Add `native_auth_owns_launches_with_a_populated_catalog` in
`tests/session_launch_contract.rs`: seed unusable preferred saved accounts plus
native login A; exercise bare and Task-owned launch/resume under both providers,
including a sandboxed headless Task. Assert native marker A, unchanged credential
files/catalog, no account network requests, no account env overrides, same resumed
thread, complete receipts and readable historical account labels. Seed one older
transcript in its account home and check transcript-only adoption. Exercise remote
launch preparation with a stand-in SSH destination: no Claude/Codex credential
payload, resident auth controls launch. Empty catalog and absent native login
reach the native client's own success/login failure respectively.

Add `native_retry_policy_matrix` in the agent unit tests for bare/Task ×
interactive/headless. A classified
rejected turn traverses configured order only in headless; explicit pins stop;
exhaustion stops without polling. Both interactive cases dispatch once and retain
provider/thread/refusal. A successful answer containing quota words, or a quota
snapshot unrelated to a rejected turn, dispatches once and completes normally.
Cross-harness continuation retains previous history and outcomes.

Checks use isolated homes and stand-ins, with inherited `LF_*` removed at the
runner boundary and stdin closed. No paid provider or production login is used.

- `cargo nextest run -p loopflow --test session_launch_contract -E 'test(native_auth_owns_launches_with_a_populated_catalog)' --test-threads 1` — passed; all four provider/Task cases, native API keys, passive quota snapshots and native missing-login diagnostics.
- `cargo nextest run -p loopflow --lib -E 'test(agent::tests::native_retry_policy_matrix) | test(harness::fact_writer::tests::a_codex_turn_retains_its_request_peak) | test(detached_lf_keeps_native_authentication_environment) | test(lf::tests::native_login_controls_are_absent_and_service_auth_remains) | test(server::schema::tests::)' --test-threads 1` — 12 selected rows passed; retry policy, receipts, native env, CLI and generated schema.
- `cargo nextest run -p loopflow --lib --test integration --test golden_prompt --test agent_tests -E 'test(transcript::tests::old_account_transcripts_resume_with_native_auth_without_copying_credentials) | test(documented_commands::) | test(auth_tests::service_auth_survives_subscription_removal) | test(session_cli_tests::session_cli_uses_one_truthful_resolution_contract) | binary(golden_prompt) | test(model_unavailable_result_falls_through_or_stops_without_spending_quota)' --test-threads 1` — 9 passed; transcript-only adoption, unchanged native credentials, historical attribution, service auth, docs, Sessions, prompts and real model fallback.
- `target/debug/lf server describe --json` — regenerated `tests/fixtures/dto/server_schema.json` under a temporary `LF_HOME`; Rust and Swift validate the same schema.
- `scripts/test_desktop.sh --filter 'ServerProtocolTests|DTOFixtureTests' -Xswiftc -gnone` — 77 passed; Desktop compiled and tests ran with WindowServer denied.
- `uv run python scripts/check_architecture.py` — passed; complete owner/table/provider/process coverage.
- `uv run --project website python scripts/render_architecture_html.py` — regenerated the architecture page.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` — passed.
- `uv run pytest python/tests/test_release_automation.py -k 'cron_bootstrap or release_cron'` — 8 passed; cron no longer requires a managed login. `py_compile` on the edited native/planning probes and `bash -n scripts/bootstrap-cron-host.sh` passed.
- `EXPLAIN QUERY PLAN UPDATE provider_session_accounts SET isolated=0 WHERE provider=?1 AND provider_session_id=?2` on 10,000 synthetic rows — primary-key index lookup; no scan.
- `cargo package --locked --allow-dirty -p loopflow` — passed; the extracted archive builds independently.
- `lf context` — scratch within 12,000 tokens and 98,304 bytes; rerun after the final notes.
- `git diff --check` — passed after implementation; rerun after the final notes.

Gate runs the changed-path suites, including the final native terminal/remote
launch contracts, parser tests, context hooks, transport/receipt callers, store
query plans and website docs; reuse the passing focused and package checks above. The earlier native
terminal/resume and remote-dispatch fixture rows passed using native login A;
no account credential payload was sent. CI owns the full matrix.

## Shipping docs — write-docs step

Native-auth behavior is written in place and checked against implementation:

- `docs/subscriptions.md`: native login, homes, transcript-only adoption, retry
  matrix, remote/rental setup and surviving service authentication.
- `docs/config.md`, `docs/sessions.md`, `docs/security.md`, `docs/ops.md`,
  `docs/troubleshooting.md`, `docs/lf-reference.md`: selection, recovery and
  destination authentication, without subscription routing or reset waits.
- `README.md`, `docs/index.md`, `docs/work.md`, `docs/telemetry.md`: entry-point
  examples and replay authentication.
- `docs/dev/codebase.md`, `docs/dev/execution.md`, `docs/dev/machines.md`,
  `docs/dev/reference.md`, `docs/dev/reference-code.md`,
  `docs/dev/reference-models.md`, `docs/dev/reference-processes.md`,
  `rust/loopflow/src/harness/README.md`, `rust/loopflow/tests/README.md`:
  ownership and acceptance guidance. Historical cutover reports remain history.

The website check now asserts the native-auth/retry contract. Product docs omit
private extraction and policy research. Saved copy corrections remain in place.

The write-docs preview command returned “No Desktop Files view attached; nothing
opened.” Docs remained at their paths; no alternate editor was opened.

## Private extraction context

The standalone port is at `/Users/jack/src/subscription-switcher`, initial main
commit `9864d7c`. Its location, policy research and private repository stay out of
public product docs.

## Live operation — 2026-11-15

Requested: “meanwhile, a lot of my codex task sessions are on a 1% account right
now. can we basicallg restart yhem all now (they should all go onto the one with
balance that you are on)”. Selected unfinished Codex Task Sessions on
`manabot-eng`; explicit native resume onto `jack@loopflow.studio` in Rex, no
message sent. Verified all 11 active on the funded account, each with a new process and
the same native thread; zero unfinished Task Sessions remain on the low account. Initial piped resume lacked a terminal; restored in Rex.

Second inventory correction: the initial `session list` used its default 100-row
limit. Full `--all --headless --limit 0` inventory contained 717 rows and found
15 more unfinished Codex Sessions on `manabot-eng`, including nine Manabot
Sessions and MTG-347. Restarted all 15 with explicit funded login, preserving
native threads and sending no message. Full inventory then verified all 15
active on `jack-42d1021d3f2d`, each with a new process and unchanged native
thread; zero unfinished Codex Sessions remained on `manabot-eng`. MTG-347 is
Task-associated, not bare.
