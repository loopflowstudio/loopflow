# Make OpenCode GLM SSE disconnects observable and recoverable — PR 2: durable failure evidence

## Problem

PR #1020 (commit `85c6956e4`, merged 2026-07-16) landed the core fix for the
OpenCode/GLM SSE hollow-body failure: content-based hollow detection
(`opencode_hollow_body`), decode-gap distinction (`opencode_decode_gap`),
disconnect turn-closure (`opencode_disconnected`), the 4-case fake-SSE matrix,
and `backup_agent` recovery routing with replay-safety gating and fencing.

What it did **not** land is the directive's durable-evidence requirement:

> "Record model, provider, endpoint class, timing, last valid event, and
> terminal error without logging credentials or raw auth."

Today `ConversationEvent::Error` carries only `{ code, message }`. The
phase-aware reason string ("before the turn produced any output" vs
"mid-stream after partial output" vs "disconnected") is human-readable but
unstructured. The rest of the evidence the directive names — which model, which
provider, which endpoint class died, when the stream started and ended, what the
last valid parsed event was, and what the terminal error class was — lives only
in ephemeral `tracing::warn!` logs and the raw `provider.jsonl` SSE stream.
Neither is queryable from the durable receipt without spelunking.

Who benefits: anyone investigating a hollow body after the fact. Today the
root-cause question ("was this transport EOF, a harness decode gap, a timeout,
or account/model routing?") requires correlating three sources by timestamp.
After this PR, the `conversation.jsonl` receipt and `lf runs` output name the
root cause from one structured record.

### The affected receipts (reproduction corpus, from `~/.lf/loopflow.db`)

The directive says "Preserve the affected run/process/Task receipts as the
reproduction corpus; do not infer the cause from the provider name alone." The
receipts are preserved in the ledger + `~/.lf/traces/`. Three failure shapes:

1. **Decode-gap hollow body** — launch `18754b05` (2026-07-16 06:54, pre-fix):
   `conversation.jsonl` has **1 event** (the user_input prompt);
   `provider.jsonl` has **858 lines** of stdout ("I'll review the full diff…")
   that the mapping layer dropped. `agent_launches.outcome = 'completed'`,
   `capture_status = 'complete'`. The model produced tokens; the harness mapped
   none. This is the `opencode_decode_gap` case — and it was marked `completed`
   because the pre-fix mapping trusted `session.status: idle`.
2. **Prompt-only hollow body** — launch `7cba313c` (2026-07-18 04:26):
   `conversation.jsonl` has **1 event** (user_input "testing");
   `incomplete_reason = "provider conversation not captured by Loopflow"`;
   `capture_status = 'prompt_only'`. No assistant output, no provider events
   captured. The turn was sent but the provider produced nothing reachable.
3. **Open turn / capturing** — launches `df4f45cb`, `0b2e7561`, `e170c903`,
   `c7b09fbd`, `71c50d8c`, `36513102`, `d363383a`, `4ea281d0` (all
   `outcome = 'running'`, `capture_status = 'capturing'`, never ended): the
   SSE stream died mid-turn and the turn was never closed. Pre-fix these left
   orphaned open turns; post-fix `close_orphaned_turn` closes them `Failed`.

The root cause is **not GLM-specific**. It is a measurement gap in the harness
mapping layer: `session.status: idle` was treated as proof of completion without
checking content. PR #1020 fixed the mapping; this PR makes the failure
self-describing in the durable record.

## The demo

Force a disconnect on a real opencode body, then:

```
lf runs --launch <launch-id>   # or lf runs showing the failed turn
```

The `error` line now carries structured evidence:

```
error  Error { code: "opencode_disconnected", message: "OpenCode event stream disconnected mid-stream after partial output", evidence: Some(FailureEvidence { model: "opencode/glm-5.2", provider: "opencode", endpoint_class: "harness_event_stream", stream_started_at: 1784352423000, stream_ended_at: 1784352455000, duration_ms: 32000, last_event_type: "message.part.updated", last_event_seq: 47, terminal_error_class: "stream_eof", terminal_error_message: "chunk stream ended (reqwest: EOF)", provider_output_tokens: None }) }
```

One record names the model, the endpoint that died, when it started and ended,
the last event that parsed, and the terminal error class — no credential
material, no raw auth. The root-cause class (`stream_eof` vs `hollow_idle` vs
`decode_gap` vs `connection_failed` vs `response_error_status`) is answerable
from the receipt alone.

## Approach — one structured field on the existing durable channel

`ConversationEvent::Error` is **internal to Rust**: not mirrored in Swift or
Python, not in `tests/fixtures/dto/`, not emitted by `lf --json`. It is
serialized to `conversation.jsonl` via `trace.rs::record_conversation` →
`RecordedConversationPayload::Conversation { event }`, and `lf runs` prints it
via `print_recorded_event`. That is the durable, queryable channel — and it is
the one a post-mortem reads.

Add one optional structured field to `ConversationEvent::Error`:

```rust
Error {
    code: String,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    evidence: Option<FailureEvidence>,
},
```

`FailureEvidence` carries exactly the fields the directive names, plus the
decode-gap distinguishing field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FailureEvidence {
    pub model: Option<String>,            // e.g. "opencode/glm-5.2" (from AgentConfig)
    pub provider: Option<String>,         // "opencode"
    pub endpoint_class: Option<String>,   // "harness_event_stream" | "upstream_provider"
    pub stream_started_at: Option<i64>,   // ms since epoch — when the SSE task began reading
    pub stream_ended_at: Option<i64>,     // ms since epoch — when the disconnect was detected
    pub duration_ms: Option<i64>,         // ended - started
    pub last_event_type: Option<String>,  // last successfully parsed SSE event type
    pub last_event_seq: Option<u64>,      // seq of the last parsed event (0-based chunk count)
    pub terminal_error_class: Option<String>,  // categorized (see below)
    pub terminal_error_message: Option<String>, // sanitized reqwest Display, no auth
    pub provider_output_tokens: Option<i64>,    // decode_gap: proves the model produced tokens
}
```

`#[serde(default)]` is safe here: `ConversationEvent` is **not** a mirrored DTO
(verified: no Swift/Python mirror, no `tests/fixtures/dto/` entry), so the
AGENTS.md DTO rule against `#[serde(default)]` does not apply. The
`skip_serializing_if` keeps old conversation logs reading cleanly — absent
fields stay absent, present fields decode.

### Terminal error classes (the root-cause axis)

The directive asks to distinguish "transport EOF, harness decode, timeout/
cancellation, or account/model routing." One field, one vocabulary:

| `terminal_error_class` | Means | Where it's set |
|---|---|---|
| `stream_eof` | SSE chunk stream returned `None` (clean EOF mid-stream) | `opencode.rs` sse_task, `chunk == None` |
| `read_error` | `response.chunk().await` returned `Err` (network/transport) | `opencode.rs` sse_task, `Err(err)` |
| `connection_failed` | Initial GET to `/event` failed to connect | `opencode.rs` sse_task, `request.send()` `Err` |
| `response_error_status` | GET succeeded but returned non-2xx | `opencode.rs` sse_task, `error_for_status` `Err` |
| `hollow_idle` | opencode reported `idle` with no content (upstream truncation mapped to idle) | `opencode_mapping.rs` `complete_hollow_turn`, `HOLLOW_BODY_CODE` |
| `decode_gap` | opencode reported `idle` with `output_tokens > 0` but no mapped content | `opencode_mapping.rs` `complete_hollow_turn`, `DECODE_GAP_CODE` |

`timeout/cancellation` surfaces as `read_error` (reqwest timeout) or
`stream_eof` (opencode server killed mid-turn by `stop()`/interrupt — but
`shutdown_requested` suppresses the error in that case, so a real timeout is
`read_error`). `account/model routing` surfaces as `response_error_status`
(401/403 from the upstream) or `hollow_idle` (the model routed to nothing). The
class + the `last_event_type` together resolve the directive's four-way
classification from one record.

### Sanitization (no credentials, no raw auth)

The terminal error message is derived from `reqwest::Error`'s `Display`, which
can include the request URL. The opencode server runs on `127.0.0.1:<port>` —
no credentials in that URL. But the upstream GLM endpoint URL or an auth header
could appear in a redirect/chained error. The sanitization rule:

- **Strip `Authorization` header values** from any error string (regex:
  `(?i)authorization:\s*\S+` → `authorization: [redacted]`).
- **Strip bearer tokens** (`Bearer \S+` → `Bearer [redacted]`).
- **Strip query-param tokens** (`(token|key|access_token|api_key)=\S+` →
  `\1=[redacted]`).
- The model and provider come from `AgentConfig` (loopflow-owned), not from the
  provider response — no credential risk there.

The sanitization runs once, at the point the evidence struct is built, before it
enters the event channel. A unit test proves a synthetic error carrying a fake
`Authorization: Bearer secret` and `?api_key=abc` is redacted in the
`terminal_error_message` that reaches `conversation.jsonl`.

### Capture sites

Three sites emit disconnect-class `ConversationEvent::Error` today; each gets
the evidence field:

1. **`opencode.rs::send_disconnect_error`** (the SSE task) — the harness's own
   `/event` stream dropped. Evidence: model + provider from `AgentConfig`,
   `endpoint_class = "harness_event_stream"`, `stream_started_at` = when the
   SSE task began, `stream_ended_at` = now, `last_event_type`/`last_event_seq`
   from `ReaderState`, `terminal_error_class` per the table above,
   `terminal_error_message` = sanitized reqwest error or "chunk stream ended
   (clean EOF)".
2. **`opencode_mapping.rs::complete_hollow_turn`** — opencode reported `idle`
   with no content. Evidence: model + provider from `AgentConfig`, (passed into
   the mapping — see below), `endpoint_class = "upstream_provider"`,
   `terminal_error_class = "hollow_idle"` or `"decode_gap"`,
   `provider_output_tokens` = the usage's `output_tokens` (for decode_gap
   proof). No reqwest error here — the stream didn't drop, the model did.
3. **`opencode_mapping.rs::map_error`** (the `session.error` event) — opencode
   itself reported an error. Evidence: model + provider, `endpoint_class =
   "upstream_provider"`, `terminal_error_class = "session_error"`,
   `terminal_error_message` = opencode's error message (sanitized).

`ReaderState` gains three tracked fields for the evidence: `last_event_type:
Option<String>`, `last_event_seq: Option<u64>`, `turn_started_at:
Option<i64>`. `map_event` updates `last_event_type`/`last_event_seq` on every
accepted event. The SSE task reads them when building the disconnect evidence.

**Model/provider reach the mapping layer** via `ReaderState::new` — extend it
to take `model: Option<String>` and `provider: &'static str` (the harness
name), set at construction in `opencode.rs::start_inner`. Today
`ReaderState::new(session_id)` knows nothing of the agent; the evidence needs
it. This is the one signature change that ripples — `process_fake_sse` and the
tests construct `ReaderState` directly and take the new args.

### Why not a new event variant or a DB column

| Channel | Pro | Con | Verdict |
|---|---|---|---|
| `ConversationEvent::Error.evidence` (chosen) | Durable in `conversation.jsonl`, visible in `lf runs`, one record | Touches every `Error { code, message }` match arm (add `..`) | **Chosen** — the receipt is where a post-mortem looks |
| New `ConversationEvent::FailureEvidence` variant | No existing matches break | New event type, ordering vs `Error`, second record to correlate, new rendering | Rejected — splits one failure across two events |
| Tracing only | No code change | Ephemeral, not durable, not queryable | Rejected — fails "Record" |
| New `agent_turns` column | Queryable in SQL | Migration + DTO-mirror risk + the evidence is per-failure not per-turn | Rejected — heavy, wrong grain |

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Is `ConversationEvent::Error` a mirrored DTO? | **No.** Verified: no Swift/Python mirror (`grep -rl ConversationEvent swift/ python/` → empty), not in `tests/fixtures/dto/`. It is internal to Rust, serialized to `conversation.jsonl` only. | Safe to add `#[serde(default)]` field. The AGENTS.md DTO rule does not apply. |
| Will adding a field to `Error` break existing match arms? | Every `ConversationEvent::Error { code, message }` match needs `..` or the new field. Counted **25 sites** (`grep -rn "ConversationEvent::Error {" rust/loopflow/src/`): 16 match arms (some already use `..`) + 9 construction sites (`.send`/`.push`). Spread across `opencode.rs`, `opencode_mapping.rs`, `task/runner.rs`, `project_session/runner.rs`, `flowloop/wave.rs`, `harness/mod.rs`, `conformance_tests.rs`, `claude.rs`, `codex.rs`. | Mechanical: match arms add `..`; construction sites add `evidence: None` (or `Some(...)` at the 3 capture sites). The enum is `#[non_exhaustive]`. |
| Does the evidence actually land in `conversation.jsonl` durably? | Yes. `trace.rs:1362-1376` `record_conversation` → `append_payload(RecordedConversationPayload::Conversation { event })`. `task/runner.rs:399` and `project_session/runner.rs:368` call `capture.record_conversation(event.clone())` for every event including `Error`. | The chosen channel is the durable receipt — verified end to end. |
| Is `lf runs` the right read surface? | `runs.rs:864` `print_recorded_event` prints `event.event_type()` + `Debug` of the event for `Conversation { event }`. New fields appear in the `Debug` output automatically (derived). | No `lf runs` change needed; `Debug` derive covers it. A follow-up can format it prettier. |
| Do real affected receipts exist? | Yes — `18754b05` (decode-gap: 858 provider lines, 1 conversation event, marked completed pre-fix), `7cba313c` (prompt_only), 8 `running`/`capturing` launches (open turns). All in `~/.lf/loopflow.db` + `~/.lf/traces/`. | The reproduction corpus is preserved. The root-cause writeup replays `18754b05` as the headline case. |
| Is `backup_agent` configured for the product wave? | **No.** `grep -rn backup_agent wave/` → empty. `wave/product/GOAL.md` frontmatter has no `backup_agent`. The recovery machinery (`classify_disconnect_recovery`) exists but routes to `AllowRetry`/`Stop`, not `HandoffToBackup`. | The directive says "route the next generation through the configured backup provider" — but no backup is configured. This is a **wave-config decision**, not a code change. PR 2 flags it in the root-cause writeup and the `Done when`; configuring `backup_agent: claude:opus` in `wave/product/GOAL.md` is an operator step, not shipped in code. |
| Can a reqwest error leak credentials? | The opencode `/event` URL is `http://127.0.0.1:<port>/event` — no credentials. But a chained/upstream error or redirect could carry an `Authorization` header or `?token=` query param in its Display. | Sanitize at build time: redact `Authorization`, `Bearer`, and `token/key/api_key` query params. Unit test with a synthetic secret-bearing error. |
| Does the fake-SSE matrix (PR #1020) already prove the evidence? | No. It asserts `assert_no_false_completed`, `assert_every_started_turn_closed`, `assert_disconnect_error_present` — i.e. the code is present and the turn closes. It does **not** assert the evidence fields are populated. | PR 2 extends the matrix: each of the 4 cases asserts the `evidence` fields are populated and correct (model, endpoint_class, terminal_error_class, last_event_type). |
| Does `opencode_mapping` know the model? | No — `ReaderState::new(session_id)` takes only the session id. The model lives in `AgentConfig.agent` (e.g. `opencode:glm-5.2`), available in `opencode.rs::start_inner` but not passed to the mapping. | Extend `ReaderState::new` to take `model: Option<String>` + `provider: &'static str`. One signature ripple; `process_fake_sse` and tests updated. |
| Is the operator 10-body run a code step? | No — it is an operator step (the directive's "finish with ten real GLM Product bodies"). PR 2 ships the observability that makes it checkable, but does not run the 10 bodies. | `Done when` names it as operator; the code `Done when` is the evidence fields + tests + replay writeup. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Record evidence in tracing structured fields only | No event change | Ephemeral — `tracing` is not the durable receipt. A post-mortem hours later cannot read it. Fails "Record." |
| New `ConversationEvent::FailureEvidence` variant | No match breakage | Splits one failure across two events (the `Error` + the `Evidence`), forcing every consumer to correlate by position. Worse, not better. |
| Store evidence in a new `agent_turns.failure_evidence_json` column | SQL-queryable | Migration + the grain is wrong (evidence is per-failure-event, not per-turn; a turn can have a hollow close AND a disconnect). The conversation log is already the right grain. |
| Enrich only the `message` string with all evidence as text | No type change | Unstructured — back to spelunking strings to find the model/timing. The directive names structured fields. |
| Put evidence on the `TurnCompleted` event instead of `Error` | Turn-scoped | The `Error` is the canonical failure record consumers match on (`drain_turn_failure_reason` reads it). `TurnCompleted` is the turn boundary; coupling evidence to it doubles the surface. |

## Key decisions

- **One optional field on the existing durable channel.** `ConversationEvent::Error.evidence: Option<FailureEvidence>`. Internal type, no DTO mirror, forward-compatible serde. The receipt is where a post-mortem looks.
- **Six terminal error classes, one vocabulary.** `stream_eof`, `read_error`, `connection_failed`, `response_error_status`, `hollow_idle`, `decode_gap`. The class + `last_event_type` resolves the directive's four-way root-cause axis (transport EOF / harness decode / timeout-cancel / routing) from one record.
- **Sanitize at build time, not at read time.** The evidence struct never carries credential material. Redact `Authorization`, `Bearer`, and token query params before the event enters the channel. Unit test with a synthetic secret.
- **`backup_agent` stays a wave-config knob, not a code default.** PR #1020 built the machinery; PR 2 does not configure it for the product wave. The root-cause writeup flags that the product wave has no backup configured, so disconnects today route to `AllowRetry`/`Stop`. Configuring `backup_agent: claude:opus` in `wave/product/GOAL.md` is an operator decision — it changes which provider spends money on recovery, which is not a code PR's call.
- **The operator 10-body run is an operator step.** PR 2 ships the observability that makes the run checkable; it does not run the 10 bodies. The `Done when` names it separately.
- **No Swift/Python change.** `ConversationEvent` is not mirrored. The evidence is visible in `lf runs` via the `Debug` derive. A prettier `lf runs` format is a follow-up, not this PR.

## Scope

**In scope**
- `FailureEvidence` struct + `evidence: Option<FailureEvidence>` on `ConversationEvent::Error`.
- `ReaderState` tracks `last_event_type`, `last_event_seq`, `turn_started_at`; takes `model` + `provider` at construction.
- Three capture sites build the evidence: `send_disconnect_error` (SSE task), `complete_hollow_turn` (mapping), `map_error` (session.error).
- Sanitization of `terminal_error_message` (Authorization/Bearer/token-params redaction) + unit test.
- Extend the 4-case fake-SSE matrix to assert evidence fields per case.
- New test: synthetic credential-bearing error → evidence is redacted.
- New test: decode-gap case asserts `provider_output_tokens > 0` in evidence.
- Root-cause replay writeup in `scratch/` replaying the `18754b05` receipt from durable evidence.

**Out of scope**
- Configuring `backup_agent` for the product wave (operator/wave-config decision).
- The operator 10-body GLM run (operator step — PR 2 makes it observable, does not run it).
- Any Swift/Python/Mac change (`ConversationEvent` is not mirrored).
- A prettier `lf runs` evidence format (follow-up; `Debug` derive is enough to prove durability).
- New `agent_turns`/`agent_launches` columns (the conversation log is the right grain).
- Unifying the `StreamEvent`/`ConversationEvent` parser stacks (filed separately, per the spend-recorders design).

## Done when

1. **`cargo test -p loopflow`** covers: the 4-case fake-SSE matrix asserts
   `evidence` is populated on every disconnect (`model`,
   `endpoint_class`, `terminal_error_class`, `last_event_type` correct per
   case); a synthetic credential-bearing error is redacted in
   `terminal_error_message`; the decode-gap case asserts
   `provider_output_tokens > 0`; the hollow-idle case asserts
   `terminal_error_class = "hollow_idle"`.
2. **`cargo fmt` + `cargo clippy -- -D warnings`** clean.
3. **No `ConversationEvent::Error` match arm is left exhaustive** without `..`
   (the field is optional; old logs decode).
4. **Root-cause writeup** in `scratch/` replays launch `18754b05` from durable
   evidence: 1 conversation event (prompt), 858 provider lines (unmapped),
   `completed` pre-fix → explains the decode-gap mechanism and why content-based
   detection (PR #1020) now catches it.
5. **Operator (out of code scope):** ten real GLM/OpenCode Product bodies, zero
   hollow successes, every forced disconnect carries `evidence` with a named
   `terminal_error_class` and `last_event_type`. PR 2 makes this checkable; the
   operator runs it.
6. **Operator (out of code scope):** configure `backup_agent` in
   `wave/product/GOAL.md` if the product wave wants disconnect-class failures to
   hand off rather than retry/stop. Flagged in the writeup; not shipped in code.

## Measure

- **Baseline:** `ConversationEvent::Error` carries `{ code, message }` only.
  Reproduce: `lf runs` on any failed opencode launch shows `error  Error {
  code: "opencode_disconnected", message: "…" }` — no model, no timing, no
  last event, no terminal error class.
- **After:** `lf runs` on a forced-disconnect launch shows the full
  `FailureEvidence` struct. Every disconnect-class error across the fake-SSE
  matrix and the operator's 10-body run carries a `terminal_error_class` and a
  `last_event_type`. Zero credential strings in any `conversation.jsonl`
  evidence field (verified by grep over the test corpus).

## Wave alignment

Serves the product wave's **loopflow-api** project (Linear `d19956b2`): "the
product contract for goal-authored computation … evidence all share one
coherent model." A hollow body that reports healthy breaks the contract; a
failure that doesn't name its root cause makes evidence incoherent. This PR
closes the gap between "the body is visible as failed" (PR #1020) and "the
receipt names why, durably" (this PR).

Advances the KR: *"Task loops earn trust by streak: every dispatched task loop
either lands its PR unattended or stops with an actionable non-convergence
record — zero silent stalls."* An actionable record names the cause. Today the
record says "disconnected"; after this PR it says "decode_gap, last event
session.status, model opencode/glm-5.2, 32s into the stream" — that is
actionable in a way "disconnected" is not.

**New risk introduced:** none. The evidence is additive (optional field on an
internal type), the sanitization is tested, and no recovery behavior changes.
The only behavioral change is that `conversation.jsonl` grows structured fields
on failure events — which is the point.

## Root-cause replay: launch `18754b05` (the decode-gap hollow body)

From `~/.lf/loopflow.db` + `~/.lf/traces/`:

- **`agent_launches`**: `id = 18754b05…`, `provider = opencode`,
  `model = opencode/glm-5.2`, `outcome = completed`, `capture_status = complete`,
  `conversation_event_count = 1`, `conversation_bytes = 24680`.
- **`conversation.jsonl`** (1 line): `seq 0`, `user_input op=initial`, text =
  "Generate a PR title and body for the changes on this branch." — the prompt,
  and nothing else. No `TurnStarted`, no `TextDelta`, no `TurnCompleted`, no
  `Error`. The mapping layer produced zero conversation events from the turn.
- **`provider.jsonl`** (858 lines): `seq 0` stdout = "I'll review the full diff
  and key files to write an accurate PR." — the model produced reasoning. `seq
  857-858` stderr = Rust source lines (the model was reading files). The
  provider stream was rich; the mapping captured none of it.

**The mechanism:** opencode's `/event` SSE stream carried `session.status:
active` → (model work) → `session.status: idle`. The pre-fix mapping saw
`idle`, called `complete_turn(Completed)`, and the turn was done — except the
`message.part.updated` events carrying the model's text/reasoning were never
mapped (a mapping gap for this event shape, or the events arrived in a shape the
mapping didn't recognize). The result: a `completed` launch with zero assistant
output — a hollow body.

**Why content-based detection (PR #1020) catches it now:**
`complete_hollow_turn` fires when `idle` closes a turn with
`turn_substantive == false`. If the mapping still drops the content events,
`turn_substantive` stays false, and the turn closes `Failed` +
`opencode_hollow_body` (or `opencode_decode_gap` if usage reports output tokens)
— never `Completed`. The body is visible as failed, the flow step does not
advance, and the recovery routing decides retry vs. backup vs. stop.

**What this PR adds to the receipt:** after the fix, the same failure produces
`Error { code: "opencode_decode_gap", evidence: Some(FailureEvidence { model:
"opencode/glm-5.2", provider: "opencode", endpoint_class: "upstream_provider",
terminal_error_class: "decode_gap", provider_output_tokens: <the tokens the
usage reported>, last_event_type: "session.status", ... }) }`. The
`provider_output_tokens > 0` is the proof that the model produced content the
harness failed to map — distinguishing a mapping regression from a hollow model
turn. That distinction is the directive's "harness decode" vs "transport EOF"
axis, resolved from the receipt.

## Implementation status

- **Slice 1 (Observability):** Done in PR #1020.
- **Slice 2 (Fake-SSE matrix):** Done in PR #1020; PR 2 extends it with evidence
  assertions.
- **Slice 3 (Recovery routing + fencing):** Done in PR #1020.
- **Slice 4 (Durable evidence — this PR):** Implemented in the working tree
  (uncommitted, ahead of base `cc9c65a19`).
  - `FailureEvidence` struct on `ConversationEvent::Error.evidence` — landed.
  - `ReaderState` tracks `last_event_type`, `last_event_seq`, `turn_started_at`,
    `model`, `provider` — landed.
  - Three capture sites build evidence: `send_disconnect_error` →
    `disconnect_evidence` (harness stream drop), `complete_hollow_turn`
    (hollow_idle / decode_gap), `map_error` (session_error) — landed.
  - `sanitize_error_message` redacts `Authorization`/`Bearer`/token params —
    landed, with a **bug fixed this session**: the bearer regex `\S+` ate the
    trailing comma that the authorization-header regex uses as its stop
    boundary, so `Authorization: Bearer <tok>, token=abc123` let the
    authorization regex consume `token=abc123` before the param redaction ran.
    Fixed to `[^,\s]+` so the comma boundary survives. The redaction test now
    passes.
  - Evidence assertions added this session to the 4-case fake-SSE matrix
    (`assert_harness_disconnect_evidence` checks `provider`,
    `endpoint_class`, `terminal_error_class`, `last_event_type`,
    `last_event_seq`, timing on every case), the hollow-idle test
    (`terminal_error_class = "hollow_idle"`, `endpoint_class =
    "upstream_provider"`), the decode-gap test (`terminal_error_class =
    "decode_gap"`, `provider_output_tokens == 42 > 0`), and the session_error
    test (`terminal_error_class = "session_error"`). The `process_fake_sse`
    helper now builds evidence via `disconnect_evidence` instead of sending
    `evidence: None`, so the matrix exercises the real evidence path.
  - `cargo fmt` + `cargo clippy -- -D warnings` clean. All 36
    `harness::opencode*` tests pass. The 139 broader `cargo test -p loopflow
    --lib` failures are pre-existing under headless `LF_RUN_ID` (`no such
    table: work_placements` and similar env-dependent SQLite/journal/wave
    failures — flagged in wave memory), not caused by this PR.
  - Root-cause replay writeup: this doc's "Root-cause replay" section above.
  - Operator 10-body run + `backup_agent` config remain operator steps.
