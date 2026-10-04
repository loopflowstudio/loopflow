# Resume the current worktree

Implementation plan — 2026-10-04. Product decisions accepted from Jack Heart; mechanisms are kickoff choices, not publication approval.

Jack requested: “`lf resume` should pick up the last interactive session in the current worktree”; recency is “most recent human turn sent, ideally. otherwise opened”. Jack also requires provider IDs and “lf resume is short for lf session resume”. Interpret “code” as Codex.

## Outcome and demo

`lf session resume [id]`, shortened to `lf resume [id]`, reconnects by Loopflow, Claude, or Codex ID. Without an ID, it resumes the current checkout's last interactive Session, preserving identity, history, and account.

Open A and B in one checkout; send a message to A last. Let B produce later output, run a background skill, and open C in a sibling worktree. From a subdirectory of the first checkout, `lf resume` opens A. Explicit Claude/Codex IDs also reopen native conversations never previously connected through Loopflow.

Placement unresolved: no Wave supplied. One PR, no follow-up Tasks.

## Findings and decisions

- `lf/navigation.rs::resolve_child` now resolves the exact root shorthand `resume` → `session resume` before descendant ambiguity, for normalization and help. Flow resume remains explicit; no root handler or routing registry was needed.
- Implementation finding (2026-10-04): `owned_target` rejected completed conversations before driver admission. Opening now permits completed Conversation targets; all other operations and closed Flow reviews retain their completion checks. A completed conversation without saved native history still returns the existing connection error.
- `ops/human_session/provider_conversation.rs::{recorded,admit}` already handles recorded and native Claude/Codex IDs, ambiguity, and account routing. Reuse it through `human_session::open`.
- Inventory sorts by title and normally hides completed conversations. A separate resume candidate query preserves list behavior and scopes by `cwd`, not repository identity, which is shared across worktrees.
- Captured `user_input` includes injected prompts and steer receipts; Claude's mapper discards text echoes. SQL turn starts and generic activity cannot prove human input.
- Codex's [message-history source](https://github.com/openai/codex/blob/main/codex-rs/message-history/src/lib.rs) records session IDs and Unix-second input timestamps. Persistence can be disabled. Read this input history rather than rollout modification time.
- Anthropic's [session reader](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/sessions.py) excludes tool results, meta messages, compact summaries, and synthetic content when extracting user prompts. Apply these distinctions to timestamped main-conversation records; unsupported provenance uses opening evidence.

## Selection and source of truth

SQLite owns Session identity and eligibility; provider records supply native send times. Candidate tuples carry optional opening timestamps; a Session-ID map carries human-input timestamps, both in epoch milliseconds. A separate recency type was unnecessary. No DTO change or schema migration.

1. Select interactive candidates, including completed conversations but excluding closed/stale Flow reviews. Resolve candidate directories and current directory to physical checkout roots, memoized per distinct directory. Outside Git, compare physical directories. Missing directories never match another checkout.
2. Resolve provider homes through recorded account routing. For matching candidates, read Codex `history.jsonl` once per owning home and Claude's exact transcript through the existing locator. No account activation or provider API calls during selection.
3. Codex: latest valid `session_id`/`ts` input record. Claude: latest RFC3339 timestamp on a meaningful main-conversation user message, excluding tool-result-only records, meta/compact summaries, synthetic notifications, and known Loopflow-injected input. Agent-issued input is not human merely because its role is user. Unrecognized or ambiguous provenance uses the fallback.
4. Rank by `human_at.or(opened_at).unwrap_or(created_at_ms)`, then stable Session ID. Fallback applies per candidate. Assistant output and ingestion timestamps never affect rank.

Stream sources with bounded memory, retaining only maxima; read each needed source once. Do not log or persist message text. No daemon or recursive import of unrelated transcripts. Ignore malformed/incomplete JSONL rows; missing/unreadable native history means unavailable evidence. SQLite failures remain errors.

## Opening boundary

Persist an `Observed` Session event with payload `{type: "interactive_opened", opened_at_ms: ...}` and receipt keyed by Exec plus Session, using `retain_session_observation`. One SQL reader derives opening recency; no pointer file or extra table.

Write after process spawn and successful `ProviderClientGuard::publish` in `lf/commands/util.rs::session_command_status_with_env`, and after successful IDE dispatch in `exec_session_with_env`. Resolve Session from the existing run environment. This covers initial launch, native resume, and live Codex reconnect. Headless execution, metadata inspection, driver claims, and failed spawn/dispatch do not count.

Opened means successful interactive handoff, not eventual successful exit. Later provider failure retains that truthful event and propagates normally. Proving UI readiness would require a new handshake and is excluded.

## Interfaces and consumers

- `lf/mod.rs`: `SessionCommand::Resume { id: Option<String> }`.
- `lf/commands/session.rs`: journaled dispatch; explicit IDs bypass checkout selection. Both forms use existing open policy.
- `human_session::latest_interactive_session(store: &SharedStore, cwd: &Path) -> Result<Option<AgentSession>>`: compose SQL candidates/openings and provider recency. Filesystem reads stay out of SQLite and CLI parsing.
- `provider_conversation.rs`: private input-time readers, sharing home/path resolution. SQL projections belong in `store/sqlite/sessions.rs` with ordinary Store wrappers.
- README and command-resolution tests change. Preserve `connect` and its flags; resume adds no flags. Swift, DTOs, account authority, and Flow settlement retain their contracts.

No match: nonzero exit, “No interactive session found in this worktree”. Preserve ambiguity and connection errors; never silently choose another Session or start fresh. No-ID discovery covers recorded Sessions; explicit provider IDs can admit native conversations.

## Alternatives and exclusions

Creation order fails when returning to older conversations. Modification time lets assistant output steal focus. New prompt hooks miss historical input and expand configuration. Choose native input evidence plus the accepted opening fallback. Exclude global native-session discovery, a picker, automatic takeover, and a new ID registry.

## Delete — do not maintain

Examples/assertions for Flow progression use `lf flow resume`. The Flow handler, connect path, and provider admission tests remain.

Forbidden: sibling selection, background Sessions winning, duplicate launch/resolution authority, synthetic input treated as human, passive inspection recording openings, or selection granting Flow authority.

## Remaining review and delivery

Implementation covers the command/shorthand, recorded and native IDs, physical checkout selection, human-input readers, opening receipts, completed conversations and README guidance. No schema or DTO change. The simulated code review found the shared completed-Session rejection; opening now admits completed conversations while mutation/review checks remain intact. The existing launch fixture also needed its native observation keyed to its input so it exercises native resume.

Compression keeps candidate eligibility and opening recency in one SQL statement, removing the intermediate collection and per-candidate application queries. Provider input readers accept Sessions without opening timestamps and resolve account details only for isolated homes. The launch-boundary fixture exposed an undrained stdout pipe: a process sample showed metadata inspection blocked in JSON output, not Session locking. The fixture now captures JSON to a temporary file while polling exit; the regression passes without changing timeouts or production behavior.

Implementation, compression and upstream integration are present. Reconciliation on 2026-10-04 found no further bounded code repair: the integrated upstream changes concern instructions and planning documentation, with no conflicting resume behavior. No Wave is identified, so no Wave memory was selected or changed.

Acceptance verification is complete for publication. Remaining work is publication, followed by Jack Heart's demo of the A/B/current-worktree scenario above and native Claude/Codex IDs in real terminals. Jack's demo remains unperformed; no review completion is claimed.

Limits: missing/unsupported native input evidence falls back per Session to opening time, then creation time for older Sessions without opening receipts. Completed conversations need saved native history. IDE dispatch and provider UI readiness have not been exercised in this headless environment; the opening boundary remains successful process/IDE handoff.

Publication review found no further resume code repair. The broad library run exposed inherited `LF_FLOW_STEP` in two launch fixtures and Git process lookup failures in two chapter fixtures. All four pass under process-isolated nextest with `LF_FLOW_STEP` removed; the legacy-project fixture reports leaked handles. Preserve this harness limitation rather than claiming a clean broad cargo-test pass. No production or test code changed during publication preparation.

Check result: `cargo test -p loopflow --test session_cli_tests --test documented_commands` passes (13); `cargo test -p loopflow --lib` yields 1690 passed, 4 failed, 8 ignored; `env -u LF_FLOW_STEP cargo nextest run -p loopflow --lib -E 'test(ad_hoc_batch_launch_captures_session_without_planning_registry) | test(two_task_research_runs_leave_distinct_uncommitted_artifacts_for_the_next_prompt) | test(every_provider_mutation_recovers_on_the_same_or_a_second_home) | test(legacy_project_adoption_preserves_plans_across_lost_responses)' --no-fail-fast` passes all 4 (1 leaky); unchanged `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and review-boundary regressions retain prior passes.
