# Resume the current worktree

Implementation plan — 2026-10-04. Product decisions accepted from Jack Heart; mechanisms are kickoff choices, not publication approval.

Jack requested: “`lf resume` should pick up the last interactive session in the current worktree”; recency is “most recent human turn sent, ideally. otherwise opened”. Jack also requires provider IDs and “lf resume is short for lf session resume”. Interpret “code” as Codex.

## Outcome and demo

`lf session resume [id]`, shortened to `lf resume [id]`, reconnects by Loopflow, Claude, or Codex ID. Without an ID, it resumes the current checkout's last interactive Session, preserving identity, history, and account.

Open A and B in one checkout; send a message to A last. Let B produce later output, run a background skill, and open C in a sibling worktree. From a subdirectory of the first checkout, `lf resume` opens A. Explicit Claude/Codex IDs also reopen native conversations never previously connected through Loopflow.

Placement unresolved: no Wave supplied. One PR, no follow-up Tasks.

## Findings and decisions

- `lf resume --help` currently resolves to Flow resume. `lf/navigation.rs::resolve_child` drives normalization and help. Add one exact preferred root shorthand there before descendant ambiguity: `resume` → `session resume`. No root handler or generic routing registry; Flow resume remains explicit.
- `ops/human_session/provider_conversation.rs::{recorded,admit}` already handles recorded and native Claude/Codex IDs, ambiguity, and account routing. Reuse it through `human_session::open`.
- Inventory sorts by title and normally hides completed conversations. Add a separate resume candidate query; preserve list behavior. Scope by `cwd`, not repository identity, which is shared across worktrees.
- Captured `user_input` includes injected prompts and steer receipts; Claude's mapper discards text echoes. SQL turn starts and generic activity cannot prove human input.
- Codex's [message-history source](https://github.com/openai/codex/blob/main/codex-rs/message-history/src/lib.rs) records session IDs and Unix-second input timestamps. Persistence can be disabled. Read this input history rather than rollout modification time.
- Anthropic's [session reader](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/sessions.py) excludes tool results, meta messages, compact summaries, and synthetic content when extracting user prompts. Apply these distinctions to timestamped main-conversation records; unsupported provenance uses opening evidence.

## Selection and source of truth

SQLite owns Session identity and eligibility; provider records supply native send times. Derive internal `SessionRecency { human_at: Option<i64>, opened_at: Option<i64> }` in epoch milliseconds. No DTO change or schema migration.

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

Replace examples/assertions using bare `lf resume` for Flow progression with `lf flow resume`. Preserve the Flow handler, connect path, and provider admission tests. No other deletion.

Forbidden: sibling selection, background Sessions winning, duplicate launch/resolution authority, synthetic input treated as human, passive inspection recording openings, or selection granting Flow authority.

## Internal slices and acceptance

1. **This slice:** candidate scope, timestamp readers, opening writer/projection. Extend provider-conversation and Session-store tests with synthetic files: human input beats later output; missing input uses opening time; synthetic records do not count; late reading preserves source time.
2. Wire canonical command and shorthand through existing connection. Extend `session_cli_tests` using isolated stores/fake providers: full demo, native Claude/Codex admission, identity/account preservation, sibling/subdirectory/symlink scoping, completed conversations, review exclusion, no match, ambiguity, and launch failure. Verify canonical/shorthand help and explicit Flow resume.
3. Update README and review for duplicated authority and unrelated history scans.

Gate: `cargo test -p loopflow --test session_cli_tests --test documented_commands`; `cargo test -p loopflow --lib`; `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`. All pass headlessly. Implement runs its focused tests/build; gate owns broader verification once.

Check result: `lf resume --help` confirmed current Flow routing; local/upstream source inspection completed; no production edits or tests run.
