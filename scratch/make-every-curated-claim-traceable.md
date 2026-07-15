# W2-124 — Make every curated claim traceable to its evidence

Linear W2-124 · Project: Auditability · Wave: product · Directive v1.

## The gap (verified in code)

Curated claims carry **no stable identity and no link to evidence**:

- `MemoryAdded { fact: String }` (`wave/journal.rs:254`) — a bare string, keyed
  only by journal `seq`. `MEMORY.md` is plain Markdown.
- `PmKr { text: String, holds: bool }` (`pm/mod.rs:67`) — a proof sentence and a
  boolean, no id, no source.
- Executive Wave reports are chat turns (`TurnItem` prose) — no binding to what
  they summarize.

Meanwhile every evidence substrate **already has a durable id**:

| Evidence            | Canonical record                              | Durable id                          |
|---------------------|-----------------------------------------------|-------------------------------------|
| chat turn / report  | wave journal `TurnStarted`/`TurnItem`         | `turn_id` (derived from `seq`, one id space across restart) |
| worker report       | journal `RunCompleted` / `run_events`         | `run_id`                            |
| trace event         | `agent_turns` / `agent_launches` (SQLite)     | `AgentTurnId` / `AgentLaunchId` (UUID) |
| PM change           | Linear issue / `TaskObservation.inbox_id`     | Linear issue UUID (survives renumber) |
| PR evidence         | `TaskPr` / `GithubPr`                          | `owner/repo#N` + merge commit sha   |

So the whole task is one missing thing: **a small, stable pointer from a curated
claim to one of these records, journaled as the source of truth, and projected
into the read surfaces.** No new evidence store; no copied transcripts.

## The receipt type (one type, all curation kinds)

```rust
/// Wire type. Every field required or explicit Optional; no serde defaults.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub kind: EvidenceKind,   // chat_turn | worker_report | trace | pm | pr
    /// The canonical durable id for `kind` (turn_id, run_id, agent_turn uuid,
    /// linear issue uuid, or `owner/repo#N@<merge_sha>`). Never a line number,
    /// never a copied raw record, never a session id.
    pub reference: String,
    /// Owning wave of the referenced record. Present so cross-wave receipts are
    /// detectable; a claim's own wave is the default at authoring time.
    pub wave: String,
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, ...)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind { ChatTurn, WorkerReport, Trace, Pm, Pr }
```

A claim carries `Vec<Receipt>` — one-to-one is the degenerate case; many-to-one
is compact because each entry is an id, not a transcript.

**Why these fields survive the four required perturbations:**

- *Markdown edits* — receipts are structural (journal events / snapshot
  columns), not text positions. `MEMORY.md` is a render, not the truth; editing
  the fact prose leaves its journaled receipt bound.
- *Branch deletion* — `pr` references carry the merge commit sha, so a deleted
  branch still resolves.
- *Session succession* — references point at journal `turn_id`/trace UUID/Linear
  UUID, never at a session id.
- *Database migration* — the journal is JSONL files (not the migrated `lfd.db`);
  trace rows are UUID-keyed; Linear UUIDs are external; the PM snapshot is a
  rebuildable read model.

## Source of truth: the wave journal

Provenance is journaled, exactly like every other curated fact, so curation
never becomes "a second unaccountable truth."

- Memory fact: extend the existing write — `MemoryAdded { fact, receipts }`.
  The fact and its evidence are one write; the claim's stable id is the
  `MemoryAdded` event's `seq`.
- KR / Project claim: a new journal event `ClaimCited { claim: ClaimRef,
  receipts }` where `ClaimRef` names the Linear project id + KR index (or project
  definition). The SQLite PM snapshot is rebuilt from Linear **overlaid** with
  journaled `ClaimCited`, projecting `receipts` onto `PmKr`. Linear schema is
  untouched — provenance stays in loopflow's own record, so humans correct it
  with `lf` commands, never by editing a database or Linear.
- Wave report: the executive turn is already journaled (`turn_id`); a report
  cites its evidence with the same `ClaimCited { claim: Turn(turn_id), .. }`.

Read surfaces (MEMORY.md render, `PmShowResult`, chat/roadmap DTOs) are
**derived** from the journal + snapshot. They display receipts; they are not
where receipts live.

## Authoring & drilling (light enough to be automatic)

- `lf memory add "<fact>" --receipt chat_turn:<turn_id> [--receipt run:<id> ...]`
  — repeatable `--receipt kind:ref`, wave defaults to the current wave. Agents
  attach as they write; the fact they're summarizing is a turn/run they just saw.
- `lf receipt show <kind:ref>` — the drill primitive: resolves one receipt to its
  canonical **local** record (journal turn, `run_events` row, trace turn, snapshot
  item, PR). This is the single resolver the DTO render affordances target.
- Human provenance correction: `lf memory cite <claim> --receipt ...` /
  `lf pm cite <project> --kr <n> --receipt ...` journal a `ClaimCited` — no DB edit.

## Doctor checks (extend `lf pm doctor` diagnostics)

`pm doctor` already emits `PmSyncResult.diagnostics`; add receipt checks there and
a parallel memory sweep. Every check is a pure function of journal + snapshot rows
(matching `doctor.rs`'s `Check` discipline), so it's tested without a store.

- **missing** — a new retained fact / KR-`holds:true` claim with zero receipts
  → `fail` (after the migration grace window; see below).
- **orphaned** — a receipt whose `reference` resolves to no record → `fail`.
- **cross-wave** — receipt `wave` ≠ claim wave. Legitimate for child→parent
  citation, so `warn` + name both waves (never silently drop).
- **inaccessible** — record exists but is unreadable (trace pruned, journal
  rotated) → distinct `warn`, not conflated with orphaned.

**Migration of existing unsourced facts:** never fabricate a receipt. Grandfather
pre-contract claims as `unsourced` (a `warn`, allowed for a grace window); only
claims written after the contract lands `fail` when missing. This matches the
proof ("every *new* retained fact ... carries a receipt").

## Affected surfaces & consumers

- **Rust wire:** new `Receipt`/`EvidenceKind` DTO; `MemoryAdded.receipts`;
  `PmKr.receipts`; `ClaimCited` journal event; `PmShowResult` carries KR receipts.
  New fixtures `tests/fixtures/dto/receipt.json` (+ round-trip in each language).
- **CLI:** `lf memory add --receipt`, `lf memory log --json` (facts+receipts),
  `lf receipt show`, `lf memory cite` / `lf pm cite`, `lf pm doctor` new checks.
- **Swift (Mac/iOS/chat):** mirror `Receipt`/`EvidenceKind`; render a source
  affordance on memory facts, KR proofs, and report turns that drills via
  `lf receipt show`. No existing provenance concept — greenfield render.
- **Prompts/agents:** teach the wave/task skills to attach `--receipt` when they
  add memory or assert a KR holds.

## Absent & error states

- New fact/KR with no receipt → doctor `fail` (grace: `warn` for legacy).
- Receipt ref resolves to nothing → doctor `orphaned` fail; render shows a broken
  affordance, never hides the claim.
- Cross-wave ref → `warn` + policy note.
- Record inaccessible (pruned/rotated) → `warn`, distinct from orphaned.
- `lf receipt show` on unresolvable ref → non-zero exit + reason, no partial spoof.

## Operational boundary

- Doctor checks are pure over rows; no network in the sweep.
- `lf receipt show` is a local read (journal / SQLite / worktree); `pm` refs may
  hit Linear cache-first, bounded, never blocking.
- Authoring adds zero network calls.

## End-to-end proof

1. In the product wave, `lf memory add "workers report via the memory stream"
   --receipt chat_turn:<a real turn_id>`; assert the fact and its receipt in
   `lf memory log --json`.
2. `lf pr land` (removes `scratch/*`), then restart the wave server.
3. `lf receipt show chat_turn:<turn_id>` opens the exact journaled turn after
   land + restart. Repeat across all five `EvidenceKind`s → 20/20 sampled claims
   open the intended evidence.
4. `lf pm doctor --json` reports zero `orphaned`/`missing`/`cross-wave` failures
   for cited claims; legacy unsourced facts show as `warn`, not `fail`.
5. Cold-start MEMORY.md still reads as prose — receipts are affordances, not
   inlined transcripts.

## Serial PRs (one worktree, ordered branches)

- **PR1 — Receipt contract + memory-fact authoring/storage/read (LANDED shape).**
  `Receipt`/`EvidenceKind`/`MemoryFact` DTO + `receipt.json` fixture, round-trip
  pinned in Rust and Swift. `MemoryAdded { fact, receipts }` (serde-default
  vector = replayed-log evolution, so old journals never truncate). Authoring:
  `lf memory add --receipt kind:ref` (repeatable, parsed at the CLI boundary,
  wave-stamped). Read: `lf memory log --json` folds the journal into
  `[{fact, receipts}]`. Proven end to end by
  `add_writes_receipts_that_the_json_view_reads_back` (client → server → journal
  → read-back). **Ships the receipt type and one full curation kind's authoring
  + durable storage + data surface + cross-language mirror.**
- **PR2 (next, `receipt-resolver-doctor`) — Drill + validation.** `lf receipt
  show <kind:ref>` resolving each kind to its canonical local record (chat_turn→
  journal turn, worker_report→`run_events`, trace→`agent_turns`, pm→snapshot
  item, pr→URL). Doctor memory sweep (missing/orphaned/cross-wave/inaccessible)
  with legacy grace. This is what makes stored receipts *openable* and *checked*.
- **PR3 — Report + KR/Project claim receipts.** `ClaimCited` journal event, PM
  snapshot overlay projecting `receipts` onto `PmKr`/`PmShowResult`,
  `lf pm cite`/`lf memory cite`, `lf pm doctor` orphaned-claim checks, migration
  grace for existing KRs.
- **PR4 — Shared render affordance (Mac/iOS/chat).** Source affordance + drill on
  memory facts, KR proofs, and report turns, over the Swift `Receipt` mirror
  landed in PR1.

## Exclusions

- No new evidence store; receipts only *point at* existing records.
- No inlining of raw transcripts into curation.
- No Linear schema change; provenance lives in loopflow's journal/snapshot.
- Runtime reconciliation (W2-169), ambient Wave resolution (W2-151), and the
  Now/Roadmap view (landed W2-146) are boundaries — this task adds no parallel
  lifecycle or status model, only the receipt layer they can each render.
