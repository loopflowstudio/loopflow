# Session Context Lab

> “I don't think another CLI is the right shape. It's probably a Swift UI for
> observing graphs and stats and flamegraphs for whole sets of `lf` sessions and
> then being able to somehow jump to a session with an LLM refining that text.”

> “More like research, iterate, edit, write?”

## What to build

Add **Context Lab** to the Loopflow Mac app: a native research workspace that
turns a selected set of local Loopflow sessions into comparable statistics,
aggregate and per-session context flames, exact trace evidence, and a one-click
handoff into a fresh LLM refinement session for the selected source text.

The session set is the primary object. Instructions emerge as expensive,
frequent, duplicated, or outcome-correlated slices of real sessions; they are
not the rows of a separate admin catalog.

## Product loop

```text
session set
    ↓
aggregate context flame + comparable session lanes
    ↓
select a text/revision slice and inspect representative sessions
    ↓
start a fresh, trace-linked LLM session in a Task worktree
    ↓
review the real source diff and observe the next revision in later sessions
```

Historical sessions remain immutable. **Refine** never resumes the session being
studied: it creates a new session linked to the selected trace addresses. This
keeps evidence and intervention separate.

## The experience

### 1. Define the session set

Open **Context Lab** from the Mac app. A persistent filter rail defines the
population:

- repo and time window;
- wave, project/task when attributed, flow, and skill;
- provider/model and surface;
- completed, failed, steered, or incomplete capture;
- current instruction revision or all revisions.

The default is the current repo over 30 days. A saved research view stores only
the filter query and selected visualization mode, never copied traces or prompt
bodies.

The header reports sessions, launches, assembled turns, context tokens, median
and p95 context per turn, instruction share, cost, outcomes, steering, and
capture coverage. Every denominator is visible. Missing token or conversation
capture stays missing rather than becoming zero.

### 2. See where context went

The center canvas has two linked modes over the same selected population.

**Aggregate flame** is an icicle rooted at the session set:

```text
session set → context kind → canonical source → content revision
```

Width is attributed supplied tokens. Color identifies context kind. Opacity
encodes capture coverage, not quality. Each node also carries session count,
turn count, and its share of the selected population. Revision evidence owns
observation dates and distribution metrics; the flame does not duplicate them.
Within a kind, nodes sort by supplied-token load so the largest research targets
stay stable.

**Session lanes** render one horizontal prompt flame per launch or turn. Lanes
share a token scale; prompt assets retain assembled order. Sorting by total
context, outcome, steering, time, or selected-source share exposes outliers
without collapsing them into an average.

Selecting a flame segment cross-filters the stats, session lanes, and evidence
rail. Zooming from `skill` to `implement` to one content hash never loses the
session-set filters. Keyboard navigation and a sortable table expose the same
nodes for accessibility; the table is not a second product model.

### 3. Move from aggregate pressure to exact evidence

The evidence rail for a selected source/revision shows:

- the effective source path, precedence layers, content hash, and token size;
- its distribution across the selected session set;
- one smooth complete session, one high-context complete session, one failed or
  heavily steered session when captured, and one recent session;
- missing prompt/conversation artifacts and overall attribution coverage.

A representative row shows metadata first. **Open trace** is the explicit act
that reveals the exact assembled prompt and normalized conversation. The app
never opens prompt or conversation bodies merely because a flame segment was
selected.

### 4. Jump into refinement

**Refine source…** is available only when the selected node resolves to one
canonical, editable source revision. It opens a sheet to choose an existing
Intelligence Task or explicitly create one. External Task creation remains a
human-confirmed side effect.

The app then opens that Task workspace and launches a fresh LLM session with a
structured seed:

- session-set query and aggregate measurements;
- selected source path and exact starting content hash;
- selected flame node and why it stood out;
- representative run, launch, and turn addresses;
- prompt/conversation artifact availability;
- the constraint to inspect exact bodies only as needed;
- the objective to improve the canonical source while preserving its intent.

The session starts in the Task worktree with the existing `refine` skill and
normal Loopflow operating context. It can call the existing trace reader,
inspect source, edit the real file, validate it, and run tests. No copied prompt
database, embedded Markdown editor, or alternate publisher is introduced.

When the agent changes the file, the app returns to the existing Task Changes
view with the source diff selected. A backlink reopens the exact Context Lab
query, flame node, and evidence set that motivated the draft.

### 5. Observe the intervention

New ordinary sessions attribute the changed source to its new content hash.
Reopening the saved research view shows the revisions as separate flame nodes
under the same canonical source. Before/after displays exposure, context load,
outcomes, steering, and capture coverage only when the populations are
comparable. It never emits a synthetic quality score or asks an LLM to grade its
own prompt.

## Layout

This is a dense research instrument, not a scroll of telemetry cards.

```text
┌ Filters ──────┬ Session-set stats + aggregate flame / session lanes ─┬ Evidence ─────┐
│ repo / window │ breadcrumb · scale · sort · coverage                  │ source/revision│
│ wave / skill  │                                                       │ distributions  │
│ model/outcome │ selected population stays visible                    │ session examples│
│ saved views   │                                                       │ Open / Refine   │
└───────────────┴───────────────────────────────────────────────────────┴────────────────┘
```

Context Lab is a new Intelligence window/surface beside Telemetry. Telemetry
continues to answer machine health, spend, and codebase size. Context Lab
answers what text shaped a selected body of sessions and provides the route to
change it.

## Data structures

```swift
struct SessionSetQuery: Codable, Hashable, Sendable {
    let repoPaths: [String]
    let startedAfter: Int64
    let startedBefore: Int64
    let waves: [String]
    let projects: [String]
    let tasks: [String]
    let flows: [String]
    let skills: [String]
    let providers: [String]
    let models: [String]
    let surfaces: [String]
    let outcomes: [SessionOutcome]
    let captureStates: [CaptureState]
}

struct ContextLabSnapshot: Decodable, Sendable {
    let query: SessionSetQuery
    let coverage: ContextCoverage
    let totals: SessionSetTotals
    let aggregateRoot: ContextFlameNode
    let sessions: [SessionLane]
    let evidence: [SourceEvidence]
}

struct ContextFlameNode: Decodable, Identifiable, Sendable {
    let id: String
    let level: ContextFlameLevel
    let kind: ContextAssetKind?    // nil only for the session-set root
    let label: String
    let sourcePath: String?
    let contentSha256: String?
    let attributedTokens: UInt64
    let sessionCount: UInt64
    let turnCount: UInt64
    let children: [ContextFlameNode]
}

struct SessionLane: Decodable, Identifiable, Sendable {
    let id: String                 // launch id
    let runId: String
    let startedAt: Int64
    let outcome: SessionOutcome
    let steeringTurns: UInt64?
    let project: String?
    let task: String?
    let turns: [TurnLane]
}

struct TraceAddress: Codable, Hashable, Sendable {
    let runId: String
    let launchId: String
    let turnId: String
}

struct RefinementSeed: Codable, Sendable {
    let query: SessionSetQuery
    let selectedNodeId: String
    let sourcePath: String
    let startingContentSha256: String
    let measurements: SourceMeasurements
    let evidence: [TraceAddress]
}
```

Wire DTOs have no defaults. Rust and Swift fixtures move together.

## Boundaries and reuse

- Swift owns interaction state, cross-filtering, zoom, selection, and rendering.
  Rust owns ledger queries, canonical source/revision identity, token
  attribution, population totals, representative-session selection, and the
  flame hierarchy. Swift must not reconstruct trace joins or source precedence.
- Do not ship a separately documented `lf instructions` product. Fold the
  prototype's useful inventory/revision aggregation into the existing
  `lf context` reader or a shared Rust query used by that reader. The Mac app
  may use daemonless JSON subprocess transport, matching `RegistryQuery`; the
  user-facing experience is Context Lab.
- Reuse `CodeFlame` interaction and drawing ideas, but introduce a context-flame
  model: filesystem hierarchy and prompt hierarchy have different identities.
- Reuse `TaskWorkspaceView`, `TaskTerminalStore`, and normal Task worktrees for
  refinement. Do not build a second editor, agent host, git client, or PR path.
- Historical trace rows and prompt artifacts are immutable. Refinement links to
  them by `TraceAddress`.
- All evidence stays local. No remote telemetry, vector database, hidden prompt
  store, or LLM-authored quality score enters the design.

## Project shape

This is a measured project inside the existing Intelligence wave, not a single
CLI task and not a new wave.

### KRs

- A maintainer can define any useful local session set and reconcile Context
  Lab's totals, coverage, and flame widths with the underlying trace/context
  ledger without Swift-side joins.
- A maintainer can move from aggregate context pressure to an exact source
  revision and representative immutable sessions, then launch a trace-linked
  LLM refinement in a real Task worktree without losing the research selection.
- Landed instruction revisions accumulate natural-run evidence as separate
  flame nodes, enabling honest before/after observation with explicit coverage
  and no synthetic quality score.

### Implementation slices

1. Replace the public instruction-reader prototype with a session-set aggregate
   query under the existing context reader. Add Rust/Swift fixtures and prove
   totals, flame widths, revision identity, trace addresses, and missing data.
2. Build the read-only Context Lab window: filters, stats, aggregate flame,
   session lanes, table parity, and evidence rail over real local sessions.
3. Add explicit exact-trace opening and Context Lab deep links/backlinks.
4. Add Task selection and fresh refinement-session launch with a structured
   seed, then return to the existing Task diff/terminal experience.
5. Add saved research views and revision comparison after natural evidence
   exists for at least two revisions.

## The demo

In the installed Mac app, select the Loopflow repo over 30 days, open the
aggregate flame, and select `LOOPFLOW.md`. Sort session lanes by selected-source
share, open the heaviest complete trace, then click **Refine source…**. Choose a
real Intelligence Task; a fresh LLM session opens in its Task worktree already
grounded in the selected hash, measurements, and trace addresses. The agent
edits the canonical file, and the app shows the real Task diff with a backlink
to the unchanged research view. Run an ordinary Loopflow session from that
worktree and see the new content hash appear as a separate flame node.

Fixture-only data does not count as the demo.

## Done when

The project is done only after one continuous live journey crosses every
checkpoint.

### Research truth

- A clean app launch loads a real 30-day session set and reports session,
  launch, turn, token, cost, outcome, steering, and capture denominators that
  reconcile with the existing local ledger readers.
- Aggregate flame child widths sum to their parent after documented tokenizer
  rounding. Session lanes preserve prompt order and share one visible token
  scale. Table and flame selection are identical and keyboard accessible.
- Filters update every stat, flame, lane, and representative session as one
  atomic snapshot. Cancellation prevents a slow prior query from replacing a
  newer selection.

### Evidence truth

- Selecting `LOOPFLOW.md` reveals its exact canonical source, revision hashes,
  precedence layers, exposure distribution, and representative trace addresses
  without opening prompt bodies.
- **Open trace** reaches the exact run and launch. Missing prompts,
  conversations, token coverage, steering attribution, and zero-exposure
  revisions have explicit states; none are rendered as success or zero.

### Refinement truth

- **Refine source…** cannot proceed without one editable canonical source,
  unchanged starting hash, and a human-selected Task. A stale hash or stale
  worktree stops with a repair path before an agent edits anything.
- The new LLM session runs in that Task worktree, can retrieve the selected
  immutable traces, and receives the exact query, measurements, source path,
  starting hash, and evidence addresses. The historical session is never
  resumed or mutated.
- The agent's source edit appears byte-for-byte in the existing Task diff and
  normal `lf commit` / PR lifecycle. Returning through the backlink restores the
  same filters, flame zoom, selected node, and evidence rows.

### Learning truth

- An ordinary post-edit Loopflow session records the new source hash with no
  import or demo-only event. Refreshing Context Lab places old and new revisions
  under the same canonical source and opens an exact new-revision trace.
- Revision comparison remains unavailable with a concrete coverage explanation
  until populations are comparable; once available it shows measured context,
  outcomes, steering, and capture only.

### Shipping proof

- Rust and Swift DTO fixtures agree exactly. Existing context/trace tests,
  focused aggregation tests, Swift model/query tests, Context Lab interaction
  tests, `cargo fmt`, `cargo clippy -- -D warnings`, and the Mac build pass.
- A maintainer who did not implement the feature completes the installed-app
  demo. Their friction, any manual terminal escape, and the exact live trace and
  Task addresses are recorded before the Intelligence project closes.
