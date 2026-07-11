# Complete local run records

> “Just recording the actual prompt given and ideally logging everything we
> possibly can for an entire run. You can actually just look into any run and
> see everything that was said by either party.”

> “My biggest concern is more just context quality.”

> “We’re just talking about my own machine right now. As long as it would fit
> on my machine, we don’t have to worry for a long time.”

## What to build

Persist a complete, local, provider-independent record of every new agent
launch and turn: the exact prompts Loopflow supplied, the provenance and token
weight of every context asset, every user/assistant/tool event Loopflow can
observe, provider usage, lifecycle, and pointers to the vendor’s own session.

This branch is backend-only. It supplies the durable evidence and JSON readers
that a later Loopflow Mac branch will use to browse past sessions and graph
average context size by wave, skill, provider, and asset kind.

## The win

Land this branch, use Loopflow normally for two weeks, then run:

```bash
lf context --days 14 --wave intelligence
lf trace <run-id>
lf trace <run-id> --events
```

The first command shows launch/turn counts plus average, p50, and p95 supplied
context for the wave, then the asset kinds contributing those tokens. The
second shows the process tree and every agent launch with exact prompt paths,
context totals, capture completeness, usage, and provider-session identity.
The third renders the complete recorded conversation and tool history. No
vendor transcript, raw SQLite query, or worktree that still exists is required.

After two weeks, `lf context --json --days 14 --wave intelligence` is one stable
payload the Mac app can graph without reinterpreting prompt assembly.

## Why this is the next slice

Most primitives already exist but do not meet in one record:

- `write_prompt_log` preserves prompt bytes under `~/.lf/logs`, but `lf trace`
  finds them by scanning filenames rather than a durable relationship.
- `ContextBreakdown` computes per-source and per-document tokens before launch,
  prints them, and discards them.
- `ConversationEvent` already normalizes Claude, Codex, and OpenCode turns,
  messages, reasoning summaries, commands, file edits, tools, usage, and errors.
- The legacy one-shot agent path already sees raw stdout/stderr and normalized
  `StreamEvent`s.
- `~/.lf/output` has an append primitive and a reader, but no production writer;
  this machine currently has zero output logs.
- `run_events.context` exists in migration 057 but no row type, writer, or reader
  uses it. All 1,613 measured rows are empty.

Storage is not the constraint. Codex plus Claude session stores occupy 2.98 GB
and gzip to 998 MB. The measured 30-day rate projects to roughly 16 GB/year raw
or 5 GB/year compressed; even the unusually active last week projects to 45
GB/year raw or 15 GB/year compressed against 201 GiB free. Persist first.
Compression, content-addressed bodies, and rotation are later optimizations.

### Current branch starting point

This worktree already contains an unintegrated `rust/loopflow/src/trace.rs` and
`061_trace_capture.sql` scaffold. Treat it as a head start, not as a completed
slice. It currently stores one assembly asset per whole prompt, puts context
timings on launches, writes absolute artifact paths, creates an empty raw file
unconditionally, has no multi-turn context creation, and is not wired into the
production launch/read/doctor paths. Its tables and enums also use the older
mixed kind/scope taxonomy that this document replaces.

As of 2026-07-10, the long-lived `~/.lf/lfd.db` does not list
`061_trace_capture` in `schema_migrations`. The implementing pass must check
that again before editing the migration. If still unapplied, replace the
unlanded 061 scaffold in place. If it has been applied to a durable ledger,
freeze 061 and add a 062 forward migration that preserves captured rows while
converging to this schema. Never make the answer depend on a fresh test DB.

## Scope

### In this branch

- Exact provider-facing system and task prompt capture.
- Per-turn context manifests with asset provenance, byte ranges, hashes, and
  token attribution.
- Durable normalized conversation/tool event capture for all observable
  headless agent paths.
- Best-effort provider-native event capture when Loopflow already receives raw
  lines; no vendor-directory copy.
- Explicit prompt-only/partial states for TUI, IDE, crashes, and missing vendor
  artifacts.
- SQLite indexes for launch, turn, asset, usage, and artifact metadata.
- `lf trace`, `lf context`, and `lf doctor` readers over the new contract.
- Removal of the unused `run_events.context` field and unfed output-log path.
- Tests against fresh and drifted ledgers, provider trace fixtures, nested
  processes, failures, interruption, and deleted artifacts.
- README examples for the new CLI behavior.

### Not in this branch

- Mac UI or Swift DTOs.
- A controlled eval harness or synthetic tasks.
- Import/backfill of historical Codex or Claude sessions.
- Copying entire vendor session directories.
- Compression, deduplication, retention knobs, or automatic deletion.
- Remote telemetry, upload, sync, or a results server.
- Semantic judging of whether an instruction was good. This branch records the
  evidence that later human and product workflows inspect.

## Concepts

Keep a 1:1 mapping with what actually happens:

- **Trace**: one `run_id`, shared by nested `lf` processes.
- **Process**: one `process_id`, already represented by `run_events`.
- **Agent launch**: one provider session or one-shot vendor process. A process
  may launch several agents; one launch may contain several turns.
- **Turn**: one user input plus the provider events and usage it causes.
- **Context snapshot**: what Loopflow supplied for one turn. The initial turn
  is fully enumerable; resumed provider history may only have provider-reported
  totals, and must say so.
- **Context asset**: one attributable region of a system/task prompt. Its kind
  says what it is (instructions, goal, memory, document, diff, message); its
  scope says where it came from (global, provider, repo, wave, project, task,
  step, or user). Keep these axes separate so “goal” can be compared across
  wave, project, task, and delegated-child boundaries.
- **Conversation record**: an append-only normalized event stream, plus the raw
  provider lines Loopflow happened to observe.

Do not pretend `process_id` identifies an agent launch or that one launch has
one turn. Those shortcuts would make session browsing and wave averages wrong.

## Storage layout

Large bodies live on disk. SQLite owns searchable metadata and relationships.

```text
~/.lf/traces/
  <run_id>/
    <process_id>/
      <launch_id>/
        conversation.jsonl
        provider.jsonl
        turns/
          0001-system.md
          0001-task.md
          0002-task.md
```

- `conversation.jsonl` is Loopflow’s stable, normalized record.
- `provider.jsonl` contains provider-native stdout/stderr or notifications only
  when Loopflow already observes them. It may be absent.
- Prompt files contain the exact strings passed in the provider-facing system
  and task positions. They are not reconstructed later from current files.
- Later turns omit `system.md` when Loopflow did not resend a system prompt.
- Source content is recovered from byte ranges in these exact prompt files;
  context assets do not need separate body copies.
- Artifact paths stored in SQLite are relative to `~/.lf/traces`; resolve them
  through one traversal-safe path helper. `provider_session_path` is the sole
  optional external path and is never treated as a Loopflow-owned artifact.
- Directories are mode `0700`; files are `0600`. Everything is local and
  gitignored. Capture may contain pasted secrets or tool output, so no content
  is printed unless the user explicitly requests prompts/events.

Keep `.lf/prompts/` only for runtime files a vendor must read. Stop writing new
durable duplicates to `~/.lf/logs`; the trace directory becomes the one durable
home. Existing logs remain untouched and are not imported.

Do not write transcript or prompt blobs into SQLite.

## Database schema

Use the next available migration (`061_trace_capture.sql` on this branch;
`059_bus` and `060_provider_token_oauth_client_id` already exist).
Migration numbers are identities: if another migration lands first, renumber
forward rather than editing an applied migration.

```sql
CREATE TABLE trace_capture_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    required_after BIGINT NOT NULL
);

INSERT INTO trace_capture_meta (id, required_after)
VALUES (1, unixepoch());

CREATE TABLE agent_launches (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    started_at BIGINT NOT NULL,
    ended_at BIGINT,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL,
    wave TEXT,
    flow TEXT,
    skill TEXT,
    provider TEXT NOT NULL,
    model TEXT,
    surface TEXT NOT NULL,
    capture_status TEXT NOT NULL CHECK (
        capture_status IN ('capturing', 'complete', 'partial', 'prompt_only')
    ),
    incomplete_reason TEXT,
    outcome TEXT NOT NULL CHECK (
        outcome IN ('running', 'completed', 'failed', 'interrupted')
    ),
    artifact_dir TEXT NOT NULL,
    conversation_path TEXT NOT NULL,
    provider_events_path TEXT,
    provider_session_id TEXT,
    provider_session_path TEXT,
    conversation_event_count BIGINT NOT NULL,
    conversation_bytes BIGINT NOT NULL
);

CREATE INDEX idx_agent_launches_run ON agent_launches(run_id, started_at);
CREATE INDEX idx_agent_launches_process ON agent_launches(process_id, started_at);
CREATE INDEX idx_agent_launches_wave ON agent_launches(wave, started_at);

CREATE TABLE agent_turns (
    id TEXT PRIMARY KEY,
    launch_id TEXT NOT NULL REFERENCES agent_launches(id),
    ordinal BIGINT NOT NULL,
    provider_turn_id TEXT,
    started_at BIGINT NOT NULL,
    ended_at BIGINT,
    status TEXT NOT NULL CHECK (
        status IN ('running', 'completed', 'failed', 'interrupted', 'partial')
    ),
    input_op TEXT NOT NULL CHECK (
        input_op IN ('initial', 'message', 'steer', 'queued')
    ),
    context_coverage TEXT NOT NULL CHECK (
        context_coverage IN ('assembled', 'provider_total_only', 'unknown')
    ),
    tokenizer TEXT NOT NULL,
    system_prompt_path TEXT,
    task_prompt_path TEXT NOT NULL,
    system_tokens BIGINT NOT NULL,
    task_tokens BIGINT NOT NULL,
    supplied_context_tokens BIGINT NOT NULL,
    provider_input_tokens BIGINT,
    provider_output_tokens BIGINT,
    reasoning_tokens BIGINT,
    cache_read_tokens BIGINT,
    cache_write_tokens BIGINT,
    cost_usd REAL,
    context_gather_ms BIGINT NOT NULL,
    context_render_ms BIGINT NOT NULL,
    context_persist_ms BIGINT NOT NULL,
    first_event_seq BIGINT,
    last_event_seq BIGINT,
    UNIQUE (launch_id, ordinal)
);

CREATE INDEX idx_agent_turns_launch ON agent_turns(launch_id, ordinal);
CREATE INDEX idx_agent_turns_started ON agent_turns(started_at);

CREATE TABLE context_assets (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id),
    position BIGINT NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('system', 'task')),
    kind TEXT NOT NULL CHECK (kind IN (
        'operating_instructions', 'surface_instructions',
        'provider_instructions', 'repo_instructions', 'skill_instructions',
        'direction', 'goal', 'memory', 'chat', 'summary', 'document',
        'scratch', 'diff', 'clipboard', 'user_message', 'assembly'
    )),
    scope TEXT NOT NULL CHECK (scope IN (
        'global', 'provider', 'repo', 'wave', 'project', 'task', 'step', 'user'
    )),
    label TEXT NOT NULL,
    source_path TEXT,
    included_by TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    byte_start BIGINT NOT NULL,
    byte_end BIGINT NOT NULL,
    bytes BIGINT NOT NULL,
    isolated_tokens BIGINT NOT NULL,
    attributed_tokens BIGINT NOT NULL,
    PRIMARY KEY (turn_id, position)
);

CREATE INDEX idx_context_assets_kind ON context_assets(kind);
CREATE INDEX idx_context_assets_hash ON context_assets(content_sha256);

CREATE TABLE context_decisions (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id),
    position BIGINT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN (
        'operating_instructions', 'surface_instructions',
        'provider_instructions', 'repo_instructions', 'skill_instructions',
        'direction', 'goal', 'memory', 'chat', 'summary', 'document',
        'scratch', 'diff', 'clipboard', 'user_message', 'assembly'
    )),
    scope TEXT NOT NULL CHECK (scope IN (
        'global', 'provider', 'repo', 'wave', 'project', 'task', 'step', 'user'
    )),
    label TEXT NOT NULL,
    source_path TEXT,
    decision TEXT NOT NULL CHECK (decision IN (
        'included', 'excluded', 'summarized', 'stat_only', 'truncated',
        'deduplicated'
    )),
    reason TEXT NOT NULL,
    original_bytes BIGINT,
    original_tokens BIGINT,
    asset_position BIGINT,
    PRIMARY KEY (turn_id, position),
    FOREIGN KEY (turn_id, asset_position)
        REFERENCES context_assets(turn_id, position)
);

CREATE INDEX idx_context_decisions_decision ON context_decisions(decision);
```

Drop the unused `run_events.context` column in the same forward migration and
remove it from schema validation. It has no production data or API. The
migration must preserve every existing `run_events` row and its three indexes;
the drifted-ledger test compares row count and summed terminal usage before and
after migration.

`agent_turns` is the source for per-turn usage. Existing cumulative
`run_events` usage remains the process-boundary snapshot consumed by `lf usage`.
The writer feeds both from the same `TurnUsage`; `lf doctor` reconciles the sum
of new turn usage with the terminal cumulative boundary instead of allowing two
independent interpretations.

`required_after` prevents pre-migration history from making capture coverage
permanently red. New processes that *enter the launch gate* after that timestamp
must have a launch row. Gate entry, not the `skill` node, is the test — see
“Capture scope: what is owed a launch” for why orchestrators and
externally-hosted skills are out of scope rather than failures.

## Rust data structures

Put capture ownership under a new `trace` module, not in the product UI or
provider adapters:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct AgentLaunchId(String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct AgentTurnId(String);

#[derive(Debug, Clone)]
pub struct TraceCaptureContext {
    pub run_id: LfdId,
    pub process_id: LfdId,
    pub repo: PathBuf,
    pub worktree: PathBuf,
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextChannel { System, Task }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextCoverage { Assembled, ProviderTotalOnly, Unknown }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextAssetKind {
    OperatingInstructions,
    SurfaceInstructions,
    ProviderInstructions,
    RepoInstructions,
    SkillInstructions,
    Direction,
    Goal,
    Memory,
    Chat,
    Summary,
    Document,
    Scratch,
    Diff,
    Clipboard,
    UserMessage,
    Assembly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextScope { Global, Provider, Repo, Wave, Project, Task, Step, User }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextDecisionKind {
    Included,
    Excluded,
    Summarized,
    StatOnly,
    Truncated,
    Deduplicated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextAsset {
    pub position: u32,
    pub channel: ContextChannel,
    pub kind: ContextAssetKind,
    pub scope: ContextScope,
    pub label: String,
    pub source_path: Option<String>,
    pub included_by: String,
    pub content_sha256: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub bytes: u64,
    pub isolated_tokens: u64,
    pub attributed_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextDecision {
    pub position: u32,
    pub kind: ContextAssetKind,
    pub scope: ContextScope,
    pub label: String,
    pub source_path: Option<String>,
    pub decision: ContextDecisionKind,
    pub reason: String,
    pub original_bytes: Option<u64>,
    pub original_tokens: Option<u64>,
    pub asset_position: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSection {
    pub kind: ContextAssetKind,
    pub scope: ContextScope,
    pub label: String,
    pub source_path: Option<String>,
    pub included_by: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct RenderedPromptChannel {
    pub text: String,
    pub tokens: u64,
    pub assets: Vec<ContextAsset>,
}

#[derive(Debug, Clone)]
pub struct PreparedTurnContext {
    pub system: Option<RenderedPromptChannel>,
    pub task: RenderedPromptChannel,
    pub decisions: Vec<ContextDecision>,
    pub coverage: ContextCoverage,
    pub tokenizer: &'static str,
}

#[derive(Debug, Clone)]
pub struct ProviderInvocation {
    pub context: PreparedTurnContext,
    pub argv: Vec<String>,
}

#[derive(Debug)]
pub struct ActiveAgentCapture {
    pub launch_id: AgentLaunchId,
    pub initial_turn_id: AgentTurnId,
    // Owns the append writers and terminal-state transitions.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedConversationEvent {
    pub schema_version: u32,
    pub seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub turn_id: Option<AgentTurnId>,
    pub payload: RecordedConversationPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawProviderRecord {
    pub schema_version: u32,
    pub seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecordedConversationPayload {
    UserInput { op: String, text: String },
    Conversation { event: ConversationEvent },
    LegacyText { stream: String, text: String },
    LegacyTool { name: String, summary: String },
    CaptureError { message: String },
}
```

DTOs and disk records get no serde defaults. Every absent field is explicitly
`Option`. Public types derive `Debug`; growing enums are `#[non_exhaustive]`.
The envelope `turn_id` is Loopflow’s stable turn id; vendor turn ids remain
inside the provider event and the indexed `provider_turn_id` column.
`ProviderInvocation.argv` participates in launch but is not persisted: capture
provider/model/surface and observable events, never authentication environment,
credentials, or command arguments that exist only to carry them.

`ContextBreakdown` should not survive beside the new manifest as a second
calculation. Derive the existing terminal header from `PreparedTurnContext` and
its assets, then delete the old aggregate implementation.

## One enforced launch gate

All production paths that can spend provider tokens must call one
`begin_agent_capture(context, invocation)` gate. It publishes the prompt
artifacts and metadata, then returns `ActiveAgentCapture`; only after that may
the caller invoke `launch_agent`, `Harness::start`, or the TUI/IDE handoff.
Capture is not an optional callback on `AgentConfig`.

Migrate and inventory the direct CLI/skill path, ops path, resident wave
harness path (`prepare_harness_turn`/`run_harness_pass`), later steer/message
turns, and interactive handoff. Keep the low-level `Harness` trait usable in
provider conformance tests, but keep raw start helpers crate-private and make
every production caller hold an active capture. The public production API
should make bypassing the gate harder than using it.

## Context accounting

The manifest must describe the exact provider-facing prompt, not an estimate
assembled independently from `PromptComponents`. Capture at the final
`ProviderInvocation` boundary after harness-specific changes such as Claude’s
structured-reply system instructions, provider wrappers, or system-prompt-file
selection. `prepare_launch_prompt` produces attributed source sections; the
provider invocation builder finalizes strings and returns the exact
`PreparedTurnContext` that capture persists and the vendor receives.

Refactor the prompt formatter to emit ordered rendered sections. Each section
knows its kind, scope, label, source path, and inclusion mechanism. Rendering
concatenates those sections and records their byte ranges in the final channel.

Do not flatten Loopflow-generated goals into `LaunchPromptInput.message` before
attribution. Replace that single ambiguous input at its producers with ordered
`PromptSection`s: CLI speech becomes `user_message/user`; a resident wave seed
becomes `goal/wave`; a project or task seed becomes `goal/project` or
`goal/task`; delegated intent becomes `goal/step` or `summary/step`. Migrate the
wave, project, task, flow, and inline launch call sites together. This avoids
having to scrape `<lf:...>` tags after rendering and makes “what parent intent
did this child receive?” directly queryable. Provider-added wrappers become
`provider_instructions/provider` sections during finalization.

Token attribution has two values:

- `isolated_tokens`: `cl100k_base` count of the asset slice alone.
- `attributed_tokens`: the increase in token count when the asset is appended
  to the rendered prefix. These values must sum exactly to the channel total.

If separators or wrapper text are not owned by a semantic asset, emit an
`assembly` asset. Never hide unexplained tokens in rounding.

Record `tokenizer = "cl100k_base"`. This is Loopflow’s supplied-context measure
and matches the prompt budgeter; it is not claimed to equal the vendor’s
private tokenizer. `provider_input_tokens` remains a separate reported measure
that may include resumed history, cache behavior, and provider framing.

Every asset also records SHA-256 and byte range. That supports later
content-addressing and lets a reader prove the asset still matches the exact
prompt file without copying each source body.

Track context creation decisions, not only what survived. When Loopflow has
considered a concrete candidate, record whether it was included, excluded,
summarized, converted to diff stat, truncated, or deduplicated and why. Include
original bytes/tokens only when the content was already read; do not scan the
whole repo merely to measure exclusions. Record decisions at existing policy
boundaries such as native-instruction deduplication, document limits, diff
tiering, summaries, ignored/non-text files, and duplicate paths. This is what
lets a future context-quality investigation distinguish “the parent never sent
it” from “the assembler deliberately left it out.”

Also retain gather, final render, and persistence durations. Context creation
must be observable as a pipeline, not only as a final byte count.

## Capture lifecycle

### 1. Establish identity before launch

Create `TraceCaptureContext` explicitly. CLI launches derive it from the active
journal context. Resident/wave launches derive it from their control run and
body process; do not rely on thread-local environment leaking across async
tasks. A new launch gets a UUID `launch_id` and creates its artifact directory.

The core launch record is required. Failure to create the SQLite row, prompt
files, or context assets aborts before vendor spend begins with an actionable
error. A missing record must not be silent.

SQLite and the filesystem cannot share a transaction. Use a recoverable
two-phase publish: write prompt artifacts into a private staging directory,
flush and atomically rename it to the final launch directory, then commit the
launch/turn/assets/decisions in one SQLite transaction. If the database commit
fails, remove the newly published directory best-effort and abort launch. If
the process dies between rename and commit, `lf doctor` reports the orphan
directory; no provider has started, so there is no unrecorded spend.

### 2. Persist the exact initial turn

`prepare_launch_prompt` carries attributed sections into the provider builder;
the resulting `ProviderInvocation` returns exact system/task bytes and the
final `PreparedTurnContext`. Persist those bytes and all assets transactionally
before calling the provider. Here “transactionally” means the two-phase
artifact publish plus SQLite commit above, not a fictitious cross-store
transaction. Create the `agent_launches` and initial
`agent_turns` rows with `capturing`/`running` states.

In-repo `.lf/prompts` files may still be written when a vendor needs a file.
Their content must be byte-identical to the durable trace prompt.

### 3. Record user input

Append `UserInput` before `Harness::send_input` or one-shot process spawn. Store
whether the operation was initial speech, a normal message, live steer, or a
queued message. A user input that fails delivery remains recorded with the turn
ending failed; history must not pretend it was never attempted.

### 4. Record normalized provider events

For modern harnesses, insert a fan-out between the provider sender and the
existing consumer. The fan-out appends every `ConversationEvent` in order, then
forwards the unchanged event to `flowloop::wave` or the resident runtime. One
writer sees every event; provider adapters do not each grow filesystem logic.

Where a modern adapter receives a provider-native JSON notification before it
normalizes it, send that payload through an optional raw-record sink owned by
the same capture writer. The adapter may identify the provider stream but may
not open files. When an SDK does not expose its original frame, record no fake
raw event: the normalized stream remains the durable contract and
`provider_events_path` stays absent.

Persist emitted reasoning summaries/deltas when the provider supplies them.
Do not infer or claim hidden reasoning.

For the one-shot `engine::agent` path, wrap every observed raw stdout/stderr
line as `RawProviderRecord` in `provider.jsonl`, map `StreamEvent` into the
stable recorded payload, and retain full assistant text/tool
summaries/usage/result. Raw capture closes the gap where the legacy mapper is
less expressive than `ConversationEvent`.

### 5. Record subsequent turns

A provider session may accept several messages. Create one `agent_turns` row
for each delivered input. When Loopflow assembles a new prompt, record full
assets with `context_coverage=assembled`. When the provider owns resumed
history and Loopflow only knows the new user input, record that asset plus the
provider-reported input total and set `provider_total_only`. Never label hidden
history as a fully enumerated context manifest.

### 6. Finish honestly

At turn completion, write usage and terminal status, flush the event files, and
update sequence boundaries. At launch completion, store event count/bytes and
one of:

- `complete`: every observable prompt/input/provider event reached disk.
- `prompt_only`: TUI/IDE handoff left Loopflow before conversation events.
- `partial`: capture began but an event/file write failed or the stream ended
  without a complete boundary.

Run success and capture completeness are independent. A failed agent run can
have `capture_status=complete` and `outcome=failed`.

Conversation append failure must not kill an already-running provider process.
Warn once, append a `CaptureError` when possible, mark the launch partial, and
make `lf doctor` fail. Do not silently continue for two weeks with empty data.

Serialize each event into one buffer plus newline and issue one append while
holding the launch writer lock. Flush after each record so a process crash
leaves every acknowledged record readable; call `sync_data` at turn and launch
terminal boundaries before committing the matching SQLite state. Readers
ignore one unterminated crash-tail record, report it, and mark capture partial.

### 7. TUI and IDE handoff

Persist exact prompts/context before handoff. Record provider and any announced
session id/path, then mark `prompt_only` unless a later importer observes the
session. No historical importer is part of this branch. Readers say “provider
conversation not captured by Loopflow” and may expose the vendor pointer.

## Backend readers

### `lf trace`

Change `lf trace <run-id> --json` from a bare span array to one explicit
envelope:

```rust
#[derive(Debug, Serialize)]
pub struct TraceDto {
    pub run_id: String,
    pub spans: Vec<SpanDto>,
    pub launches: Vec<AgentLaunchDto>,
    pub turns: Vec<AgentTurnDto>,
    pub assets: Vec<ContextAssetDto>,
    pub decisions: Vec<ContextDecisionDto>,
}
```

No current product consumer calls `lf trace --json`; update Rust fixtures and
keep `lf usage --json` unchanged.

Human output adds one block per launch/turn with capture state, context totals,
provider usage, prompt/event paths, and missing-artifact reasons. It does not
print sensitive prompt or transcript bodies by default.

Add:

```text
lf trace <run-id> --events
lf trace <run-id> --events --launch <launch-id-prefix>
lf trace <run-id> --events --jsonl --launch <launch-id-prefix>
```

Human `--events` folds normalized records into readable user/assistant/tool
history. `--jsonl` streams the stored event objects for the selected launch;
it does not load a 100 MB session into one in-memory array. Keep `--json` for
the single metadata envelope so every machine-readable mode has one shape.

### `lf context`

Add:

```text
lf context [--days N] [--wave NAME] [--repo PATH] [--json]
```

Human output has two sections:

1. per-wave launch/turn counts, average/p50/p95 **initial assembled context**,
   average provider input across all turns where reported, and complete/
   partial/prompt-only counts;
2. asset-kind contribution totals and averages for those initial assembled
   contexts, followed by follow-up-turn supplied input as a separate line.

Do not mix tiny follow-up messages into the initial-context average. “Average
context for a wave” means one initial assembled context per launch. Provider
input across all turns is a second measure because it may include vendor-owned
history that Loopflow cannot enumerate.

JSON emits one payload:

```rust
#[derive(Debug, Serialize)]
pub struct ContextDatasetDto {
    pub days: u32,
    pub turns: Vec<ContextTurnDto>,
    pub assets: Vec<ContextAssetDto>,
    pub decisions: Vec<ContextDecisionDto>,
}
```

Rows carry run/process/launch/turn ids, timestamp, repo, wave, flow, skill,
provider/model, coverage, capture status, supplied totals, provider totals,
asset metadata, inclusion decisions, creation timing, and artifact availability.
Future Mac code can compute daily stacks, per-wave averages, asset flames, and
excluded-context audits from this payload without opening prompt files or
duplicating token rules.

### `lf doctor`

Add a `capture` check after lineage:

- every process that entered the launch gate after `required_after` has an agent
  launch, and every process whose terminal `run_events` row reports provider
  spend has one; orchestrators and externally-hosted skills that never entered
  the gate are out of scope, not failures (see “Capture scope: what is owed a
  launch”);
- every assembled turn has prompt files and at least one context asset;
- asset attributed tokens reconcile exactly to system/task totals;
- complete launches have parseable, monotonically sequenced conversation
  JSONL and a terminal turn;
- Loopflow-owned DB paths are relative, traversal-safe, and resolve inside
  `~/.lf/traces`; external vendor-session paths are checked only for current
  availability;
- staged/final artifact directories with no launch row are reported as
  pre-launch orphans;
- turn usage sums reconcile with the process’s terminal cumulative boundary;
- missing raw vendor pointers are informational, not failures;
- `prompt_only` is a warning, not a fabricated complete record;
- missing normalized artifacts for a complete headless launch is a failure.

Include capture rows and bytes in the detail. Do not warn about total size yet;
there is no retention policy in this branch.

## Capture scope: what is owed a launch

The first cut of the capture check went red under normal use. Its definition of
“agent-bearing” — any post-`required_after` process with `node = 'skill'` or a
provider value — conflates three different things:

1. **In-process launches.** A headless `lf -b :` both runs a skill and invokes a
   provider in the same process. It is owed a launch, and it has one.
2. **Orchestrators and dispatchers.** `lf code`, `lf queue`, and `wave_pursue`
   emit a `skill` event but spend no provider tokens themselves; their provider
   work happens in child processes. They are *not* owed a launch of their own.
3. **Externally-hosted providers.** `lf -m codex …` shells out to an external
   CLI; a Claude-Code-hosted skill (e.g. `lf demo`) runs the provider outside
   Loopflow’s gate entirely. Loopflow never reaches `begin_agent_capture`, so
   there is nothing to capture in-process.

Keying “owed a launch” off `node = 'skill'` flags (2) and (3) as failures even
though every real in-process launch was captured. That is why the demo ledger
was green only across a pristine window of hand-run probes and red the moment a
wave did real work — while every one of the nine captured launches was still a
direct `lf -b :` probe, and no `lf code` run produced any child launch at all.

Redefine the check around the gate, not the skill vocabulary:

- **Owed set = processes that entered the launch gate.** The two-phase publish
  already stages a capture directory before the SQLite commit. A process is owed
  a launch iff it has a staged or committed capture. A committed launch satisfies
  it; a staged-but-uncommitted directory is the existing pre-launch orphan,
  reported as before. Drop the `node = 'skill'` inference; it is a proxy for the
  wrong thing.
- **Keep a positive leak check.** Independently, any process whose terminal
  `run_events` row reports provider spend but has no launch is a hard failure and
  names that exact process. This is what actually catches a gated path that
  regresses and skips capture — without flagging a dispatcher that spent nothing.
- **Classify externally-hosted providers explicitly.** A codex dispatch or a
  host-run skill either enters the gate as an `external`/prompt-only receipt
  (prompts and any vendor receipt durable, no fabricated transcript) or is marked
  out of capture scope. It never reads as an agent-bearing process missing a
  launch, and its spend is never silently dropped: if Loopflow can see the
  provider total it records a receipt; if it cannot, the launch says so.

Then confirm the dispatch path actually reaches the gate. `lf code`’s children
are separate `lf` processes that invoke Claude; each must call
`begin_agent_capture` and produce its own launch. On the demo ledger no `lf code`
run produced any child launch — proof #6 has zero live evidence. Before tuning
the check, verify a real `lf code` on a small fixture writes child launches; if
it does not, the gap is an ungated child path, and closing it is the
higher-priority fix over the doctor heuristic.

## Degradation and deletion

The product contract cannot assume Codex or Claude retain their own sessions.

- Loopflow’s normalized trace and exact prompts are durable.
- Vendor session id/path is an optional receipt, never the only transcript.
- If a user manually deletes `conversation.jsonl`, metadata and context
  aggregates still work. `lf trace` reports the exact missing path and
  `lf doctor` fails capture integrity.
- If the vendor deletes its raw session, normalized browsing remains complete
  and the vendor artifact is reported expired.
- If only provider-native raw events are missing, normalized browsing remains
  complete.
- Never turn missing data into an empty successful conversation.

No automatic rotation in this branch. When measured storage pressure arrives,
the eviction order is provider-native raw events, then large content-addressed
tool bodies; prompts, manifests, normalized user/assistant events, usage, and
lifecycle remain. That later policy must be explicit and observable.

## Implementation order

`lf code` should execute these required slices in order. Each slice leaves the
branch testable; none is optional.

### Slice 1 — schema and artifact store

- Add migration and typed lfdb rows/queries for launches, turns, assets, and
  inclusion decisions.
- Add `trace_capture_meta.required_after`.
- Add safe artifact path construction and file permissions.
- Add append/read helpers for normalized and raw JSONL.
- Drop the unused `run_events.context` column and dead row validation.

### Slice 2 — exact prompt manifest

- Replace flattened generated messages at inline, wave, project, task, and
  flow call sites with typed `PromptSection`s.
- Refactor rendering into ordered attributed sections and finalize them at a
  provider invocation boundary after all provider-specific prompt mutation.
- Replace `ContextBreakdown` with the exact `PreparedTurnContext` manifest.
- Persist prompt bytes, assets, inclusion decisions, and creation timings
  before launch.
- Derive the existing terminal context header from the manifest.
- Keep runtime `.lf/prompts`; stop new durable `~/.lf/logs` copies.

### Slice 3 — conversation capture

- Capture user inputs.
- Fan out all modern `ConversationEvent`s before runtime folding.
- Tee provider-native notifications from modern adapters that already receive
  them through the shared raw sink.
- Tee one-shot raw lines and normalized `StreamEvent`s.
- Persist turn usage and capture terminal states.
- Mark TUI/IDE launches prompt-only with provider receipts.
- Remove the unfed `~/.lf/output` writer/reader/pruner and point trace readers
  at the new artifact store.

### Slice 4 — readers and integrity

- Extend `lf trace` envelope and event reader.
- Add `lf context` human/JSON datasets and filters.
- Add `lf doctor` capture checks and usage reconciliation.
- Update README examples and TESTING.md.
- Run the real readers on `~/.lf/lfd.db`, not only temp stores.

## Tests

### Prompt and context accounting

- Every `PromptComponents` variant emits a named asset with source and scope.
- System/task prompt files are byte-identical to `AgentConfig` values.
- Asset byte ranges recover the expected exact slices.
- SHA-256 matches each slice.
- Attributed asset tokens sum exactly to channel and turn totals.
- Wrapper/separator tokens appear as `assembly`, never unexplained.
- Per-document path/token data survives for docs, scratch, diff, wave memory,
  summaries, and repo instructions.
- Included/excluded/summarized/stat-only/truncated/deduplicated decisions retain
  a reason and link to the resulting asset when one exists.
- A resumed provider-owned turn is `provider_total_only`, not assembled.

### Artifact durability

- Files and directories use private permissions.
- Concurrent nested processes write distinct launch directories.
- Concurrent event append produces parseable, ordered JSONL.
- A crash tail remains readable through its last complete line and is partial.
- Unsafe run/process/launch ids cannot escape the trace root.
- Deleting one artifact produces a named missing-artifact result.

### Provider coverage

- CLI, ops, resident-harness, and TUI/IDE fixture launches each have a durable
  launch row before the fake provider observes its first start call.
- Existing Claude, Codex, and OpenCode conformance fixtures are recorded and
  replayed through `RecordedConversationEvent` without losing event types.
- User input precedes provider turn start.
- Text, reasoning summaries, commands, tool input/output, file edits, diffs,
  usage, completion, interruption, and error events persist.
- The one-shot stream path records raw stdout/stderr and normalized text/tool/
  usage/result events.
- Modern adapter fixtures retain provider-native notifications when the adapter
  receives them, without making raw capture a prerequisite for completeness.
- TUI/IDE capture is prompt-only and never claims a transcript.

### Ledger and readers

- Fresh migration builds all constraints and indexes.
- A drifted pre-061 fixture migrates without touching historical run evidence.
- Dropping `run_events.context` preserves row count, process ids, terminal
  usage totals, and run/process/time indexes.
- `lf trace --json` round-trips the full envelope.
- `lf trace --events --jsonl` streams valid records for one launch.
- `lf context --json` filters by days/wave/repo and carries enough ids to join
  every asset to one turn and trace.
- Average/p50/p95 initial-context grouping counts each launch once, not each
  turn or asset; all-turn provider aggregates count each eligible turn once.
- `lf doctor` catches missing launch rows, bad paths, token mismatch, malformed
  JSONL, missing terminal turns, and usage disagreement.
- Pre-`required_after` runs do not make capture permanently red.

## Done when — workflow proofs

The branch is not done when tables exist. It is done when all of these user
workflows hold:

1. **Inspect a normal run.** Run a headless inline prompt that replies and uses
   one harmless tool. `lf trace <id>` names its launch and turn, shows complete
   capture, exact system/task prompt paths, context totals, tool event, usage,
   and terminal status.

2. **Read the whole exchange.** `lf trace <id> --events` shows the initial user
   request, assistant text, tool call/result, and final response in order. No
   raw vendor file is opened.

3. **Explain a prompt.** For that run, every byte of the system/task prompts is
   covered by an ordered context asset or explicit assembly asset. The asset
   token sum equals the supplied context total exactly.

4. **See parent intent.** Run a skill inside the Intelligence wave. Its context
   dataset visibly distinguishes Loopflow operating guidance, repo instructions,
   wave goal, memory, recent chat/summary, skill, and user message when present.

5. **Explain an omission.** Force a large diff into stat-only mode and include
   duplicate instruction input. The context dataset says which candidates were
   transformed or deduplicated, why, and which included asset replaced them.

6. **See multiple launches.** Run `lf code` on a small fixture. One trace shows
   separate implement/compress/lint/gate launches and turns rather than one
   overwritten context blob.

7. **Audit a step contract.** Select the gate launch from that `lf code` trace.
   Its manifest separately names the inherited task goal, gate skill
   instructions, relevant repo/wave context, and any parent summary. A reviewer
   can tell whether the step received its quality bar without reconstructing
   today’s files or reading an undifferentiated prompt blob.

8. **Graph two weeks later.** Seed fixtures for multiple days/waves, then run
   `lf context --json --days 14`. The payload supports average context per wave,
   daily initial-context totals, all-turn provider totals, and stacked
   asset-kind charts without reading files or conflating follow-up messages
   with launch context.

9. **Keep provider totals honest.** A resumed multi-turn session records the
   exact new user input and provider-reported input tokens while marking hidden
   provider history `provider_total_only`.

10. **Preserve failure.** A provider error and an interrupted turn retain every
   event observed before failure plus a terminal failed/interrupted turn. The
   launch can be capture-complete even though the work failed.

11. **Survive a crash.** Kill a fixture writer mid-turn. The next reader returns
   all complete records, calls the launch partial, and explains why; it does not
   reject the whole trace or call it complete.

12. **Degrade after deletion.** Delete a normalized conversation fixture.
    Context averages still work, `lf trace` says which artifact is missing, and
    `lf doctor` fails capture integrity. Delete only the vendor pointer target;
    normalized browsing still works and doctor does not fail.

13. **Tell the truth about interactive handoff.** Launch with TUI/IDE in a
    fixture. Prompts/assets are durable, provider receipt is retained when
    known, and the launch says prompt-only instead of showing an empty complete
    transcript.

14. **No silent outage.** Force the artifact root unwritable before launch.
    Loopflow refuses to spend provider tokens. Force an append failure during a
    running turn; the run continues, emits one warning, and doctor later fails
    the partial capture.

15. **Reconcile usage.** For a multi-turn agent launch, summed turn usage equals
    the terminal process snapshot under the existing cumulative rule. Skill and
    terminal boundaries still sum without double-counting.

16. **Stay local.** Network inspection and code search show no telemetry upload,
    analytics SDK, or remote results service. All new bodies live below
    `~/.lf/traces` and all indexes in `~/.lf/lfd.db`.

17. **Do not duplicate durable prompts.** A new run writes runtime prompt files
    only where the vendor needs them and one durable copy under the trace root;
    it does not add a second durable file under `~/.lf/logs`.

18. **Exercise the real ledger.** After focused tests, run a real two-turn
    Claude or Codex probe, then `lf trace`, `lf context`, and `lf doctor` against
    the long-lived migrated ledger. All readers succeed and the new run is
    capture-complete.

19. **Stay green under real wave operation.** Run `lf code`, `lf queue`, and a
    `-m codex` dispatch in a live wave, then `lf doctor`. Capture is green or a
    warning, never a failure, as long as every process that entered the launch
    gate has a launch. Orchestrator and externally-hosted processes that never
    entered the gate are not counted as missing launches. (First cut regressed
    here: capture flipped red the moment any orchestrator ran, and the green
    claim held only on a ledger of hand-run `lf -b :` probes. See “Capture scope:
    what is owed a launch.”)

20. **Prove multi-launch on the live ledger, not just in prose.** A real
    `lf code` on a small fixture writes child launches for its
    implement/compress/lint/gate turns into the ledger, and `lf trace` on that
    run shows them as separate launches. Proof #6 is satisfied by captured rows
    on the long-lived ledger, not only by fixture tests. If no child launch
    appears, the gap is an ungated child path, not a doctor heuristic.

21. **Name external provider spend honestly.** A `-m codex` run or a
    Claude-Code-hosted skill either records a launch (when it enters the gate) or
    is classified as an externally-hosted provider with a receipt. It is never
    reported as an agent-bearing process missing a launch and never fabricated as
    a complete in-process transcript. A gated path that spends provider tokens
    without a launch is still a hard `lf doctor` failure naming that exact
    process.

## Verification commands

```bash
cargo fmt --check
cargo test -p loopflow trace
cargo test -p loopflow journal
cargo test -p loopflow lfdb
cargo test -p loopflow harness::conformance_tests
cargo clippy -p loopflow --all-targets -- -D warnings

env -u LF_RUN_ID -u LF_PROCESS_ID cargo run -p loopflow --bin lf -- \
  -b : "Reply with one sentence, then run pwd"
cargo run -p loopflow --bin lf -- runs
cargo run -p loopflow --bin lf -- trace <printed-run-id>
cargo run -p loopflow --bin lf -- trace <printed-run-id> --events
cargo run -p loopflow --bin lf -- context --days 1 --json
cargo run -p loopflow --bin lf -- doctor --json
```

The implementing agent should substitute the emitted run id; do not hardcode a
fixture id into the real-ledger check.

## Measures

Record before/after values in the PR notes:

- percentage of post-migration gate-entering processes with a launch record
  (measured by gate entry, not the `skill` node, which includes ungated
  orchestrators and externally-hosted skills);
- percentage of headless launches capture-complete;
- percentage of assembled turns whose asset tokens reconcile exactly;
- provider coverage across Claude, Codex, and OpenCode fixtures;
- prompt preparation overhead before and after attribution;
- bytes written per provider-reported input/output token;
- total trace artifact bytes and largest launch artifact;
- `lf context --json` runtime over 30 days and over the full local history;
- `lf trace --events` time-to-first-record for the largest fixture.

Acceptance targets:

- 100% launch coverage after `required_after` for every process that enters the
  launch gate; orchestrator and externally-hosted processes are out of scope by
  definition, not by exception.
- 100% exact token reconciliation for assembled turns.
- 100% parseable normalized event files after clean completion.
- No more than 100 ms additional warm prompt-preparation latency for a typical
  context fixture; report rather than hide larger outlier costs.
- `lf context --json --days 30` completes under one second warm on the current
  machine without opening prompt/transcript bodies.
- Event reading streams; memory use does not scale with the full transcript.

## Constraints that must survive implementation

- Context quality is the reason for capture; context size is a dimension and
  guardrail, not the optimization target.
- The exact prompt passed to the provider outranks reconstructed intent.
- One run may contain many processes, launches, and turns.
- One asset/token attribution implementation feeds CLI, doctor, and future Mac.
- `run_events` remains the one process-level lifecycle and spend ledger.
- Large bodies stay on disk; searchable metadata stays in SQLite.
- No silent best-effort loss. Required pre-launch capture fails closed; mid-run
  loss marks partial and makes doctor red.
- No remote telemetry, vendor backend dependency, or vector store.
- No automatic rotation yet. Personal-machine storage is ample.
- No raw vendor-session copy. Persist what Loopflow observes and keep an
  optional vendor receipt.
- No production abstraction exists only for tests.
- No backwards-compatibility branch for pre-capture history. Old runs have no
  launch rows and honestly show that complete capture was unavailable.

## Size assessment

This exceeds the usual single-commit heuristic: expect roughly 1,500–2,500
Rust lines plus migration, tests, and docs. The user explicitly chose an
ambitious backend-only branch so two weeks of use produces a complete dataset.
Keep it one PR, but implement the four slices as reviewable checkpoints and do
not start Mac UI work. `lf code` can run directly from this document.
