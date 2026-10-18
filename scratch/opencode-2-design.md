# LOO-504: Restore OpenCode 2 operation

Working design reconciled October 17, 2026. Jack Heart's accepted outcome and
delivery authorization remain in `opencode-2-audit-brief.md`, matching the current
Task brief. The normal-operation adapter cutover is implemented; acceptance and
delivery remain incomplete. The current audit matrix and historical native
counterexamples live in `docs/reviews/opencode-2/README.md`.

## Accepted decisions — October 17, 2026

Jack Heart renewed the priority: “get opencode 2 running.” The selected scope is
fresh interactive launch, completed-session resume, explicit subsequent input,
context/tools and headless Task/Flow execution, followed by autonomous verification,
publication, landing and supported installed acceptance. Native full-screen
rendering, configured account access, LOO-415 ownership and LOO-501 diagnostic
isolation remain required. The wider Claude/Codex parity audit remains part of the
outcome; a successful startup alone does not complete it.

Interrupted captures retain failed/unknown outcomes. No automatic replay,
synthetic continuation or empty successful wake may complete them. Automatic
crash continuation is explicitly deferred, not a prerequisite for ordinary
operation. Managed native recovery injects model-visible restart guidance and has
at-least-once semantics; Jack Heart has not accepted those semantics. Future
adoption would need supported content-free resume with suspended-claim ownership,
or an explicit decision accepting the observed alternative. No such decision is
needed for the selected normal-operation repair.

## Implemented approach and evidence

One OpenCode 2 adapter uses an authenticated launch-owned private server,
API-confirmed native identity and the native terminal. Session APIs carry model
selection; current context hooks deliver instructions, scratch and skills.
The old endpoints, startup-log identity discovery, parentID grouping, export
command and legacy plugin callbacks/fixtures are replaced together. Historical
persisted receipt readers remain. No production migration was introduced for this
acceptance work.

Ordered native messages join assistant steps to admitted initiating inputs with
frozen capture/Process/Work ownership. Steers retain independent origins;
headless separate turns use the host queue. Completion requires an idle marker
and a successful nonempty answer. Repeated observations retain idempotent usage
and completion; disconnect does not invent native success. Actual native
handoff/queue acceptance remains distinct from these source and fixture proofs.

Deterministic real-provider PTY/model-endpoint evidence covers tools, fresh launch,
completed resume, initial/refreshed Unicode scratch, a large skill, headless output
and a one-step Flow. Independent configured-account acceptance covers bare launch,
editable input, a shell result, reconnect with remembered history, stable native
identity and old/new Process attribution. It found a misleading error on successful
receipts; that is fixed and covered by the receipt regression. The operator's
original observations remain in `opencode-2-operator-acceptance.md`.

Review removed duplicate launch/mapping state and history clones. Terminal setup
always signals worker closure before joining, including diagnostic-file failures;
a public launch regression covers cleanup. SSE framing preserves split UTF-8 and
mixed CRLF/LF boundaries. No additional code mismatch was established during this
reconciliation.

Gate found and repaired another startup-recovery boundary: a private server can
die before native identity exists while its Process bookkeeping remains stale.
Reopening now settles confirmed-dead attachments before requiring native identity
for interruption of live headless work. The existing startup-recovery fixture now
fails before endpoint publication, and the handoff fixture uses the shared OpenCode
2 private server. The standalone JavaScript context test also uses current hooks.
Native gate probes now cover JSON routing, explicit input after model failure and
SIGKILL without completing the interrupted capture, and editable Unicode drafts
across resize. Results and remaining native gaps are retained in the audit.

## Related changes and retained limits

The checkout includes #1585 (`40a9a697f0`): resume uses an explicit Session ID and
failed-start fixtures use private-server diagnostics. #1587 (`3111ea1cea`) routes
ordinary commands through the installed entry; isolated candidate probes therefore
pin their candidate executable. These upstream changes preserve Session/transport
contracts.

Infrastructure owns reliability through verified delivery. Its immediate Release
child's memory supplies the entry-point lesson: source or fixture success does
not prove installed operation. The configured-account probe used a disposable store;
its shared-store attempt stopped at the preexisting session-transport schema
boundary before provider startup. Supported release/materialization and installation
remain required; applying an unreleased draft to the shared store is not acceptance.

Native evidence still rejects parentID correlation, replayable-event assumptions,
native queue idle as separate-result proof and admission replay as recovery.
Historical recovery probes remain evidence, not a selected implementation path.
LOO-454 owns context-consumption accounting; LOO-502 owns upgrade continuity.

## Remaining work

1. Complete CI and remaining native acceptance against the final candidate.
   Outstanding cases are actual live-provider viewer handoff, independent steer
   and host-queued results, full native Task
   workflow, Worktree permission denial and stale-viewer cancellation,
   compaction/child-session isolation, native agent settings,
   cross-provider continuation, wider attention and auth/model/network failures.
   The audit matrix distinguishes fixture, deterministic-native and configured-account
   evidence; Claude/Codex baseline inspection is not a fresh provider run.
2. Keep unsupported and untested audit gaps explicit, with existing owning Tasks
   linked where applicable. Human rendering judgment belongs to demo/review;
   required automated checks remain headless. Automatic crash continuation stays
   an audited limitation without weakening supported-path acceptance.
3. Publish and land through the caller's delivery procedure, then coordinate
   supported release/installation with its existing owner and replay the original
   `lf -a opencode` experience. Preserve unrelated Sessions, Flows, accounts and
   checkouts. PR #1590 is published; merge and supported installed repair remain open.

Check: prior unchanged-code results retained—`uv run python scripts/test_network.py cargo test -p loopflow --lib opencode` (25 passed, 2 live ignored), `--lib agent::session::tests` (4 passed), and `--test session_launch_contract` with each filter `opencode_headless_start_and_resume_preserve_history_and_reload_policy` and `terminal_launch_and_resume_use_current_policy_and_preserve_native_identity` (1 each), with `< /dev/null`; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed. Prose reconciliation: `git diff --check` passed; `lf context` within both targets. Broader acceptance remains with gate/CI and delivery.

Publication check: `uv run python scripts/test_network.py cargo test -p loopflow --test session_launch_contract < /dev/null` passed all 19 tests; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed. Review against Task base `616d73b4dc` found no further publication blocker; native/account evidence and its limits remain as recorded above. This is a review checkpoint; affected gate suites, full CI and supported installed acceptance remain with delivery.

Gate check: `uv run python scripts/test.py --base 616d73b4dc --reuse-passing` initially reported Rust 2,666 passed/10 failed/22 skipped and website 75 passed; all ten failures now pass focused follow-ups (human-session 13, Session CLI 21, lifecycle repairs, transport resume, canonical-repository selection 6), plus data-clones 4; one nextest leaked-handle warning remains; final fmt, Clippy, Ruff, architecture scanner/21 tests, context-hook JavaScript and all three native probe modes passed; combined-tree CI, paid live tests and installed acceptance remain with delivery.
