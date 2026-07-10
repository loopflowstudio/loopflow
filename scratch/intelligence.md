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
- **Context asset**: one attributable region of a system/task prompt: operating
  guide, surface instructions, repo instructions, skill, wave goal, project,
  memory, recent chat, parent summary, docs, scratch, diff, clipboard, or user
  message.
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
- Directories are mode `0700`; files are `0600`. Everything is local and
  gitignored. Capture may contain pasted secrets or tool output, so no content
  is printed unless the user explicitly requests prompts/events.

Keep `.lf/prompts/` only for runtime files a vendor must read. Stop writing new
durable duplicates to `~/.lf/logs`; the trace directory becomes the one durable
home. Existing logs remain untouched and are not imported.

Do not write transcript or prompt blobs into SQLite.

## Database schema

Use the next available migration (`059_trace_capture.sql` on this branch).
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
        'loopflow', 'surface', 'structured_reply', 'provider_wrapper',
        'repo_instructions', 'skill', 'direction',
        'wave_goal', 'project', 'wave_memory', 'wave_chat', 'parent_summary',
        'docs', 'scratch', 'diff', 'clipboard', 'user_message', 'assembly'
    )),
    scope TEXT NOT NULL CHECK (scope IN (
        'system', 'repo', 'wave', 'project', 'task', 'step', 'user'
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
        'loopflow', 'surface', 'structured_reply', 'provider_wrapper',
        'repo_instructions', 'skill', 'direction',
        'wave_goal', 'project', 'wave_memory', 'wave_chat', 'parent_summary',
        'docs', 'scratch', 'diff', 'clipboard', 'user_message', 'assembly'
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
remove it from schema validation. It has no production data or API.

`agent_turns` is the source for per-turn usage. Existing cumulative
`run_events` usage remains the process-boundary snapshot consumed by `lf usage`.
The writer feeds both from the same `TurnUsage`; `lf doctor` reconciles the sum
of new turn usage with the terminal cumulative boundary instead of allowing two
independent interpretations.

`required_after` prevents pre-migration history from making capture coverage
permanently red. New agent-bearing processes after that timestamp must have a
launch row.

## Rust data structures

Put capture ownership under a new `trace` module, not in the product UI or
provider adapters:

```rust
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
    pub kind: String,
    pub label: String,
    pub source_path: Option<String>,
    pub decision: ContextDecisionKind,
    pub reason: String,
    pub original_bytes: Option<u64>,
    pub original_tokens: Option<u64>,
    pub asset_position: Option<u32>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedConversationEvent {
    pub schema_version: u32,
    pub seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub turn_id: Option<String>,
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

`ContextBreakdown` should not survive beside the new manifest as a second
calculation. Derive the existing terminal header from `PreparedTurnContext` and
its assets, then delete the old aggregate implementation.

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

### 2. Persist the exact initial turn

`prepare_launch_prompt` carries attributed sections into the provider builder;
the resulting `ProviderInvocation` returns exact system/task bytes and the
final `PreparedTurnContext`. Persist those bytes and all assets transactionally
before calling the provider. Create the `agent_launches` and initial
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

1. per-wave turn count, average/p50/p95 supplied context, average provider
   input, complete/partial/prompt-only counts;
2. asset-kind contribution totals and averages.

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

- every agent-bearing process after `required_after` has an agent launch;
- every assembled turn has prompt files and at least one context asset;
- asset attributed tokens reconcile exactly to system/task totals;
- complete launches have parseable, monotonically sequenced conversation
  JSONL and a terminal turn;
- DB paths remain inside `~/.lf/traces`;
- turn usage sums reconcile with the process’s terminal cumulative boundary;
- missing raw vendor pointers are informational, not failures;
- `prompt_only` is a warning, not a fabricated complete record;
- missing normalized artifacts for a complete headless launch is a failure.

Include capture rows and bytes in the detail. Do not warn about total size yet;
there is no retention policy in this branch.

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

- Add migration and typed lfdb rows/queries for launches, turns, and assets.
- Add `trace_capture_meta.required_after`.
- Add safe artifact path construction and file permissions.
- Add append/read helpers for normalized and raw JSONL.
- Drop the unused `run_events.context` column and dead row validation.

### Slice 2 — exact prompt manifest

- Refactor rendering into ordered attributed sections and finalize them at a
  provider invocation boundary after all provider-specific prompt mutation.
- Replace `ContextBreakdown` with the exact `PreparedTurnContext` manifest.
- Persist prompt bytes/assets before launch.
- Derive the existing terminal context header from the manifest.
- Keep runtime `.lf/prompts`; stop new durable `~/.lf/logs` copies.

### Slice 3 — conversation capture

- Capture user inputs.
- Fan out all modern `ConversationEvent`s before runtime folding.
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

- Existing Claude, Codex, and OpenCode conformance fixtures are recorded and
  replayed through `RecordedConversationEvent` without losing event types.
- User input precedes provider turn start.
- Text, reasoning summaries, commands, tool input/output, file edits, diffs,
  usage, completion, interruption, and error events persist.
- The one-shot stream path records raw stdout/stderr and normalized text/tool/
  usage/result events.
- TUI/IDE capture is prompt-only and never claims a transcript.

### Ledger and readers

- Fresh migration builds all constraints and indexes.
- A drifted pre-059 fixture migrates without touching historical run evidence.
- `lf trace --json` round-trips the full envelope.
- `lf trace --events --jsonl` streams valid records for one launch.
- `lf context --json` filters by days/wave/repo and carries enough ids to join
  every asset to one turn and trace.
- Average/p50/p95 grouping counts each turn once, not once per asset.
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

7. **Graph two weeks later.** Seed fixtures for multiple days/waves, then run
   `lf context --json --days 14`. The payload supports average context per wave,
   daily context totals, and stacked asset-kind charts without reading files.

8. **Keep provider totals honest.** A resumed multi-turn session records the
   exact new user input and provider-reported input tokens while marking hidden
   provider history `provider_total_only`.

9. **Preserve failure.** A provider error and an interrupted turn retain every
   event observed before failure plus a terminal failed/interrupted turn. The
   launch can be capture-complete even though the work failed.

10. **Survive a crash.** Kill a fixture writer mid-turn. The next reader returns
   all complete records, calls the launch partial, and explains why; it does not
   reject the whole trace or call it complete.

11. **Degrade after deletion.** Delete a normalized conversation fixture.
    Context averages still work, `lf trace` says which artifact is missing, and
    `lf doctor` fails capture integrity. Delete only the vendor pointer target;
    normalized browsing still works and doctor does not fail.

12. **Tell the truth about interactive handoff.** Launch with TUI/IDE in a
    fixture. Prompts/assets are durable, provider receipt is retained when
    known, and the launch says prompt-only instead of showing an empty complete
    transcript.

13. **No silent outage.** Force the artifact root unwritable before launch.
    Loopflow refuses to spend provider tokens. Force an append failure during a
    running turn; the run continues, emits one warning, and doctor later fails
    the partial capture.

14. **Reconcile usage.** For a multi-turn agent launch, summed turn usage equals
    the terminal process snapshot under the existing cumulative rule. Skill and
    terminal boundaries still sum without double-counting.

15. **Stay local.** Network inspection and code search show no telemetry upload,
    analytics SDK, or remote results service. All new bodies live below
    `~/.lf/traces` and all indexes in `~/.lf/lfd.db`.

16. **Do not duplicate durable prompts.** A new run writes runtime prompt files
    only where the vendor needs them and one durable copy under the trace root;
    it does not add a second durable file under `~/.lf/logs`.

17. **Exercise the real ledger.** After focused tests, run a real two-turn
    Claude or Codex probe, then `lf trace`, `lf context`, and `lf doctor` against
    the long-lived migrated ledger. All readers succeed and the new run is
    capture-complete.

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

- percentage of post-migration agent-bearing processes with a launch record;
- percentage of headless launches capture-complete;
- percentage of assembled turns whose asset tokens reconcile exactly;
- provider coverage across Claude, Codex, and OpenCode fixtures;
- prompt preparation overhead before and after attribution;
- bytes written per provider-reported input/output token;
- total trace artifact bytes and largest launch artifact;
- `lf context --json` runtime over 30 days and over the full local history;
- `lf trace --events` time-to-first-record for the largest fixture.

Acceptance targets:

- 100% launch coverage after `required_after` for supported headless paths.
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
