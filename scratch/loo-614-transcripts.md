# LOO-614 — Read conversations from harness transcripts and show what is retained

Status: **implemented, pending gate** (2026-10-26). Jack Heart's steer
(LOO-602 decision 6, 2026-10-26) removed the `loopflow` state and any copy. Product direction is accepted:
LOO-602 decision 4 (Jack Heart, 2026-10-26), design at
`../loopflow.loo-602-top-down-what/scratch/loo-602-tracking.md` §4 and §7.4.
Implementation choices below are reversible; open choices are in
`scratch/questions.md`.

## Problem

Providers keep conversation text. Loopflow keeps facts: who ran it, when, for
which Task, at what cost. Today you can't read a Loopflow Session's
conversation through `lf` at all. `lf history show SESSION` prints the latest
input's status, agent, cwd and context cost, and no turns. You also can't see
how much of your history is still readable or when it goes away. Claude
deletes transcripts idle longer than `cleanupPeriodDays`, 30 days by default,
and no home on Jack's machine sets it. Codex keeps rollouts indefinitely.
Nothing tells you which applies to a given Session.

Measured on Jack's machine (2026-10-26, from `agent_sessions` and the provider
homes): 5,571 Sessions record a Claude or Codex thread, and all were created
on or after 2026-10-02. Claude: 1,603 of the 1,607 with a thread still have a
transcript; 499 Claude Sessions recorded no thread. Codex: 3,964 of 3,964 are
present. The first Claude expiries arrive about 2026-11-01. A view that shows
this has a few days to be useful before history starts disappearing without
anyone seeing it.

## Demo

```
lf history show <today's claude session>   # facts header, then the turns from ~/.claude/projects/…/<id>.jsonl
lf history show <a codex session>          # same Turn form, labelled "codex transcript, account engineering"
lf history show <session whose file is gone>
#   Retention: Claude removes conversations idle 30 days (default); set cleanupPeriodDays in its settings.json to keep more
#   Transcript: gone at claude; removed after 2026-10-14
#   Turns (facts): 2026-09-12 14:02 completed · 41.2k in / 3.1k out · final answer: "…"
lf history search "cleanupPeriodDays" --since 30d   # Session · turn time · snippet, across Claude/Codex/Pi/OpenCode
lf history retention
#   claude  1,603 readable · 503 facts only · next expiry 2026-11-01 (12 Sessions), 30-day window
#   codex   3,964 readable · 176 facts only · no provider expiry
#   by account · by age (<7d, 7–30d, 30–90d, >90d) · expiring in the next 7 days: …
```

In Desktop, a **Retention** card on the Telemetry dashboard shows the same
summary from `lf history retention --json`.

## Chosen approach

### One reader module, built from the parts that already exist

Two partial readers already exist. Both get folded into one module:

- `ops/human_session/provider_conversation.rs` contains LOO-324's lookup:
  `conversation_home` (isolated account home, else the native home), `transcript`
  (Claude `projects/*/<id>.jsonl`, Codex `sessions/Y/M/D/rollout-*-<id>.jsonl`),
  `find_file`, `recorded_cwd`, `is_user_input` and `json_lines`.
- `ops/human_session/conversation_log.rs` contains row parsers (`claude_row`,
  `codex_row`, `opencode_entries`, `lf_message`, `codex_request`) that emit
  a lossy `Entry`. That output feeds cross-provider continuation, and it trims
  tool input to 200 chars and keeps only failure tails.

New crate module `rust/loopflow/src/transcript/`:

```rust
// transcript/mod.rs
pub struct Turn {                       // the one normalized form, derived when read, never written
    pub at: Option<OffsetDateTime>,     // when the request was sent
    pub request: Option<Request>,       // None for a turn the agent continued on its own
    pub items: Vec<ConversationItem>,   // existing chat::types item enum, full fidelity
}
pub struct Request { pub text: String, pub from: RequestSource }   // Person | Injected (lf:/hook/system blocks)

pub struct Locator { provider: Provider, agent_session: AgentSessionId, home: Option<PathBuf> }
pub fn locate(store: &SqliteStore, provider, &ProviderSessionRef) -> Result<Option<Locator>>  // moved conversation_home
pub fn read(locator: &Locator) -> Result<Option<Transcript>>   // None = gone; Err = present but unreadable
pub struct Transcript { pub path: Option<PathBuf>, pub modified: OffsetDateTime, pub turns: Vec<Turn> }

// transcript/{claude,codex,pi,opencode}.rs — one reader each: fn turns(source) -> Result<Vec<Turn>>
```

- **Claude**: transcript rows. `is_user_input` moves here and decides
  Person vs Injected. Sidechains and meta rows are dropped. `tool_use` and
  `tool_result` pair by id into `ConversationItem::Tool`/`Command`.
- **Codex**: `response_item` `message`/`reasoning`/`function_call`/
  `custom_tool_call` plus their `_output` rows. Requests come from
  user `message` rows, using `codex_request`'s unwrapping.
- **Pi**: the AgentSessionId is the absolute session file
  (`pi_history.rs:95`). Entries form a tree through `parentId`, so the reader
  follows the ancestor chain of the last entry, which is the active branch.
- **OpenCode**: `opencode session export <id> --standalone` from the Session's
  cwd. This is the existing path, moved here. It is the only reader that runs
  a subprocess.

`conversation_log` becomes a renderer over `Vec<Turn>`. Its trimming policy
stays and its row parsers are deleted. `human_input_times` keeps its own fast
timestamp scan, but takes `is_user_input` from `transcript::claude`.

### Retention state is derived when read, never stored

```rust
pub enum RetentionState { Provider, Facts, Unknown }
pub struct Expiry { pub policy: ExpiryPolicy, pub after: Option<OffsetDateTime> }
// ExpiryPolicy: ClaudeCleanup { days, settings: Option<path> } | Kept (Codex, Pi, OpenCode)
```

- `Provider`: the native transcript file exists. For Claude, `after` is the
  file's mtime plus `cleanupPeriodDays`, read from `<home>/settings.json` with
  30 as the default. Claude deletes at its next startup after that date, so
  the text says "removed after", not "removed on". Codex and Pi have no
  provider expiry.
- `Facts`: no transcript. Either no thread was recorded, the account home is
  gone, or the file is gone. When the policy explains the gap
  (last fact + period < now), the text says "gone at claude, removed after
  <date>". Otherwise it says "missing", with no invented cause.
- `Unknown`: OpenCode in the summary only. Probing it needs a server
  per project. `lf history show` still reads it exactly.
- No `loopflow` state: Loopflow copies no transcripts (decision 6). The
  retention view names Claude's `cleanupPeriodDays` as the way to keep more.

### Surfaces

| Command | Change |
|---|---|
| `lf history show SESSION` | Prints the existing facts header, then a **Conversation** section: each AgentSession the Session selected, oldest first, labelled with source (provider · account · path), then its turns, or the facts and expiry when gone. Other flags are unchanged. |
| `lf history show SESSION --transcript [--json]` | Prints only the conversation. `--json` emits a `SessionTranscript` DTO. `--json` without `--transcript` keeps today's input snapshot, which tests and canonical-id fixtures read. |
| `lf history search TEXT` | Accepts `--since` (default 7d), `--wave/--project/--task`, `--limit` and `--json`. SQL selects Sessions first; matching reads each transcript once, using a raw byte prefilter, then parses only files that hit. |
| `lf history retention [--json]` | Shows counts by state × provider × account × age bucket, the next expiries (the 20 soonest, plus the count within 7 days) and the expiry policy per home. |
| Desktop Telemetry | Adds a `RetentionCard` leaf view fed by `lf history retention --json`. |

Facts for a gone transcript come from existing native receipts in
`session_events`: `started`, `completed` with status, `usage`, and `output`
for the final answer. These rows are keyed by `provider_thread` and
`provider_turn`, and they also list every AgentSession a Session used. A
`SqliteStore` read helper (`session_turn_facts(session_id)`, with savepoints
per AGENTS.md) returns them grouped by thread.

The summary uses one scan per home (`HomeIndex::scan(provider, home)`, which
maps id to path and mtime) instead of a `read_dir` per Session. It shares the
file-name rules with `transcript()`. Each lookup applies the same matcher, and
neither duplicates the naming.

## Risk findings

- **Claude cleanup timing.** Cleanup works from file mtime and runs when
  Claude starts. Jack's oldest Claude file is from 2026-09-27, about 29 days
  old, which fits a 30-day default. Precedence across project, local and
  managed settings can override the home value. The reader states the
  home-level value and its source and claims no more. No lf launch passes
  `cleanupPeriodDays` (grep, 2026-10-26).
- **Format drift.** Claude 2.1.310 and Codex 0.164.0 ship weekly. Readers are
  display-only, and cost and routing come from receipts. An unknown row type
  is skipped. A file that yields zero turns while non-empty is reported as
  "unreadable by this lf (format changed?)", never as gone. Fixtures come
  from real current files with content scrubbed.
- **Codex rollouts carry the thread id in the filename.** 9,731 rollout ids
  across `~/.codex` and `~/.lf/accounts/codex/engineering`. One Session can
  match the same id in both homes after a share. `locate` picks the recorded
  home (`conversation_home`) and doesn't search others; LOO-324's `find`
  owns discovery for unrecorded ids.
- **Scale.** The summary for about 6.2k Sessions needs one SQL pass and one
  directory walk per home: about 2.5k Claude files across 6 homes and
  9.7k Codex files across 2. Target: under 1 s warm. Search over 30 days is
  bounded by bytes (Claude 2.9 GB, Codex 7.9 GB total). The prefilter keeps
  the 7-day default under about 5 s warm, and the command prints the bytes it
  scanned.
- **Pi branches.** A fork leaves sibling branches in one file. Showing only
  the active branch matches what Pi resumes.
- **Copied history** (a Session from a given-back Machine): the transcript is
  on the other Machine. It shows as facts, labelled "copied from <machine>".

## Alternatives considered

- **Route transcript rows through the live `*_mapping` decoders.** Claude
  stream-json resembles transcript rows, but Codex rollouts, Pi files and
  OpenCode exports do not match their live protocols. One decoder per
  harness is LOO-602 decision 7, a separate Task. When it lands, the decoders
  can absorb these readers. Doing it here would block this Task on that
  redesign.
- **Index transcripts into SQLite for search.** Rejected: LOO-602 forbids a
  second transcript store. A rebuildable cache stays possible later.
- **Store retention state on the Session.** Rejected: it is derived when read
  by direction, and it would go stale the moment Claude cleans up.

## Scope

**Exclusions:** any Loopflow transcript copy or opt-in Loopflow-format file,
and syncing `cleanupPeriodDays` into homes (both ruled out by LOO-602
decision 6, Jack Heart, 2026-10-26; the Task description's `loopflow` state is
superseded). Removing `--events` and raw capture (decision 3).
Merging live decoders (decision 7). Desktop conversation browsing beyond the
summary card (no Desktop transcript view exists today). Discovering
unrecorded provider conversations (LOO-324 `find`, unchanged).

**Source of truth:** provider files for content; `session_events` native
receipts and `provider_session_accounts`/`provider_accounts` for facts and
locators. `Turn`, `RetentionState` and the summary are derived views.

**Absent and error states:** no thread → Facts ("no provider conversation
recorded"). Isolated account deleted → Facts ("account home removed").
File missing → Facts with expiry reasoning. File present but unparsable →
the error names the path, and other segments still render. OpenCode CLI
missing → that segment reports "opencode not installed", and the summary
reports Unknown. Empty store → `retention` prints "No Sessions recorded."

**Affected consumers:** `conversation_log::read/continuation` (the
cross-provider continuation output must stay byte-identical for the same
transcript). `human_input_times`, used by primary selection. History CLI
help. `docs/lf.md` history section. `docs/lf-reference.md`, regenerated.
DTO fixtures plus Swift mirrors for `SessionTranscript` and
`RetentionSummary`.

## Current → target architecture

| | Before | After |
|---|---|---|
| Locate a transcript | `provider_conversation::{conversation_home, transcript, find_file}` (private to human_session) | `transcript::{locate, HomeIndex}`, used by show, search, retention, continuation and LOO-324 admission |
| Parse a transcript | `conversation_log` row parsers → lossy `Entry` | `transcript::<h>::turns` → `Turn`; `conversation_log` renders Turns |
| Classify Claude person input | `provider_conversation::is_user_input` | `transcript::claude::is_user_input` (one copy) |
| Read a conversation via lf | not possible | `history show`, `--transcript`, `search` |
| Retention visibility | none | `history retention`, Desktop card |

## Delete — do not maintain

- `conversation_log.rs`: `Entry`, `transcript_entries`, `claude_row`,
  `codex_row`, `opencode_entries`, `opencode_export` (moved), `text_of`, and
  tool/failure extraction duplicated by the readers. Its three tests become
  renderer tests over `Turn` fixtures with the same assertions.
- `provider_conversation.rs`: `transcript`, `find_file`, `recorded_cwd`,
  `conversation_home`, `json_lines`, `is_user_input` move to `transcript/`
  (moved, not copied). Their tests move with them.
- Required survivors: `admit`/`select`/`find` (LOO-324 discovery),
  `human_input_times`, continuation output.

## Forbidden outcomes

- A second parser for any harness's transcript, or a second lookup of file
  names.
- Any transcript text written to SQLite or a cache presented as truth.
- Retention state or expiry persisted.
- "Expired" claimed without a policy that explains it.
- Changing `history show --json`'s existing input snapshot shape.

## Internal slices (one PR)

All five slices are in the working tree: `rust/loopflow/src/transcript/`
(readers, locate, `scan`, `ExpiryPolicy`, `session_transcript`, `summary`),
`lf/commands/history_transcript.rs` (show rendering, `retention`, `search`),
store helpers `session_turn_facts`/`retention_sessions`, DTO fixture
`retention_summary.json`, Swift `RetentionSummary` + `RetentionCard`.

Shape after compress: `transcript::threads(store, session)` is the one
per-thread lookup (facts + harness + `Located`) that show and search share;
`Located::Found { source, home }` carries the home, so expiry policy never
re-derives it from path depth; one `walk` serves both single lookup and
`scan`; accounts are `ProviderAccountId` from the store boundary on.
`lf history show`'s five conflicting view flags reach `show`/`inspect` as one
`SessionView`.

Concept review (2026-10-26, against Jack Heart's decision 6): the model holds —
two states a person sees (at the provider, or facts only) plus `not checked`
for OpenCode in the summary, and no copy or settings sync anywhere. Its one
implement direction is done: a facts-only Claude conversation prints
`policy.describe()` (the `cleanupPeriodDays` way to keep more) above
"removed after <date>"; Codex/Pi/OpenCode facts-only conversations print
nothing extra (`history_transcript::tests::a_gone_claude_conversation_says_how_to_keep_more`). Noted, not changed:
the summary's `facts only` count includes Sessions that never recorded a
thread (499 Claude) as well as removed ones; decision 6's binary accepts
this, and `lf history show` gives the reason per conversation.

Remaining: gate (affected suites, clippy, fmt, CLI reference regeneration,
Swift tests via CI), demo on Jack's machine.

## Done when

- `lf history show` on a live Claude Session and a live Codex Session prints
  their turns in one form, labelled with source and account.
- A Session whose file is gone prints its receipts (time, status, usage,
  final answer) and a truthful expiry statement.
- `lf history retention` on Jack's machine reports about 1,603 readable
  Claude Sessions, with next expiries in early November 2026, and all Codex
  Sessions readable with no provider expiry. It finishes under 1 s warm.
- Desktop Telemetry shows the Retention card from the same JSON.
- Cross-provider continuation produces the same log as before for the same
  transcript.

## Acceptance at gate (headless)

With inherited `LF_*` unset and `< /dev/null`:

- `cargo test -p loopflow --lib transcript conversation_log`: each reader
  maps its fixture to the expected Turns, and the renderer output matches
  the pre-change continuation text.
- `cargo test -p loopflow --lib transcript::summary`: an ephemeral store with
  isolated Claude/Codex account homes, one Claude transcript (mtime 10 days
  ago), one Codex rollout, one Session whose Claude file is gone 40 days after
  its last receipt, one with no thread. Retention counts Provider 1 / Facts 2
  for Claude with `after` = mtime + 30d; the gone one reads facts and
  "removed after <date>" (see questions for why this is a lib test).
- `cargo test -p loopflow --test integration dto_fixtures` plus Swift `DTOFixtureTests`
  and `RetentionCardTests` (headless, via `xcodebuild test`), or deferred to
  CI's Swift job.
- `UPDATE_CLI_REFERENCE=1 cargo test -p loopflow --test integration reference_matches_the_compiled_cli`.

Check: `cargo clippy -p loopflow --all-targets -- -D warnings` clean; `cargo test -p loopflow --lib -- transcript conversation_log provider_conversation history_transcript session_history` → 18 passed; gone Claude conversations now print the cleanupPeriodDays policy (`cargo test -p loopflow --lib -- transcript history_transcript` → 10 passed). Integration suites and Swift → gate/CI.
