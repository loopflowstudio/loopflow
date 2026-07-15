# W2-86 — Conduct a Wave end to end from the Mac

Building the Mac into the daily conductor surface: from Now/Roadmap, shape a
Wave's direction from the Mac — inspect evidence, record trajectory, calibrate,
arrange beats, launch/steer — without dropping to a terminal.

This Task ships as **ordered serial PRs on one branch**. This note designs
**PR 1** and names the arc for the rest.

## Dependency status (resolved)

Directive v1 named W2-146 PR #920 as an explicit dependency to stack/rebase
through. **#920 is CLOSED**; the Home-control work landed as **#918**
(`57ca881`), which is an ancestor of this branch's base commit `bee9371b9`. So
the base already carries `HomeRuntime` / `WaveHomeControl` / `RoadmapView` /
`RegistryQuery.homeProbe|homeStart` and the shared `RoadmapSnapshot`. **PR 1
builds directly on main — no stacking, no rebase-through-Loopflow needed.**

## What already exists (do not rebuild)

- **Work surface** — `lf roadmap --json` → `RoadmapSnapshot`, rendered by
  `RoadmapView` as Now/Roadmap with per-Task lifecycle actions
  (run/attach/resume/interrupt) and per-Wave Home control. Launch, reattach, and
  steer are done.
- **Evidence backbone** (PR #919): `lf memory log --json` → `[MemoryFact]`
  (`{fact, receipts:[{kind,reference,wave}]}`), read **daemon-less** from the
  wave journal. `lf memory add "fact" --receipt kind:reference` writes **through
  the wave's live Home server** — there is deliberately no offline write path.
  `MemoryFact` / `Receipt` / `EvidenceKind` are already mirrored in Swift
  (`swift/Loopflow/Models/Receipt.swift`), no serde defaults.
- **Wave detail** — `WaveDetailPane` = `WavePlanView | WaveChatView`
  (objective + projects/KRs + inspector, beside durable chat).

## PR 1 — Trajectory & evidence on the Mac

The smallest coherent conduct slice that is genuinely end-to-end and not already
shipped by W2-146: **read the Wave's curated trajectory, inspect the evidence
behind each note, and record a new evidence-backed note — round-tripping to the
same state `lf memory` shows in a terminal.**

A curated memory fact *is* the Wave adjusting its own durable record; recording
one is the conduct verb "record trajectory notes" from the user story, and it is
the calibration primitive the richer KR-approval slice will build on.

### User-visible outcome

In a Wave's detail on the Mac, the user sees a **Trajectory** section: the
Wave's memory facts oldest-to-newest, each with its evidence receipts shown as
`kind:reference` chips (a `pr:` receipt is a clickable GitHub link). A compose
field records a new note; when the Wave's Home is live the note appears in the
list immediately. When the Home is not live, the section says so and points at
the existing Start-on-Home control instead of failing silently.

### End-to-end proof

One scenario crossing the source of truth and every consumer:

1. Terminal: `lf memory add "trajectory: chose X over Y" --receipt pr:loopflow/loopflow#918@57ca881 -w product`
   (Home live). Mac Trajectory section shows the note with the PR chip within one
   refresh — **CLI write → Mac read** round-trips.
2. Mac: type a note + add a `chat_turn:` receipt, Record. `lf memory log --json -w product`
   in the terminal shows the identical fact + receipt — **Mac write → CLI read**
   round-trips.
3. Stop the Home. Mac Trajectory read still renders (journal is daemon-less); the
   compose field is disabled with "Start the Wave's Home to record trajectory."

Automated: a Swift unit test over the `[MemoryFact]` → view-model mapping
(oldest-first, receipt chips, PR-link resolution, empty state), and the existing
Rust `add_writes_receipts_that_the_json_view_reads_back` already pins the
round-trip at the CLI boundary.

### Source of truth

The wave **journal** (`wave/<wave>/…` events; `MemoryAdded { fact, receipts }`).
`lf memory log --json` folds it into `[MemoryFact]`; the compiled `MEMORY.md` is
a derived checkpoint, not read here. The Mac holds **no** trajectory model of its
own — it decodes `MemoryFact`/`Receipt` via the shared Codable types and renders.

### Affected surfaces and consumers

- **Rust CLI** — none. `lf memory log --json` and `lf memory add --receipt` exist
  and are the contract; PR 1 adds no Rust.
- **Swift `RegistryQuery`** — add `memoryLog(wave:cwd:) -> [MemoryFact]`
  (daemon-less; needs the wave's origin repo path, which the Mac has as
  `wave.repo`) and `memoryAdd(wave:cwd:fact:receipts:)` (shells `lf memory add`,
  surfaces the no-server error). Both wrap the same subprocess path every other
  `RegistryQuery` read/write uses.
- **Swift Mac** — a `WaveTrajectorySection` inside `WavePlanView`'s scroll, after
  projects (least relayout: the plan pane is already the durable-record column).
  Reuses `MemoryFact`/`Receipt`, `Home` state for the compose gate, and the
  existing evidence-banner/refresh idioms in `RoadmapView`.
- **iOS** — untouched in PR 1 (parity is a later slice; the DTOs are shared, so
  no drift is introduced).

### Absent and error states

- No facts yet → "No trajectory recorded yet." (not an error).
- `lf memory log` read fails → evidence banner "Trajectory unavailable — <detail>",
  last-good list retained, consistent with `RoadmapView`'s refresh-failure idiom.
- Record with no live Home → the write's `no live listener` error is caught and
  shown as the Start-on-Home hint; the field disables rather than throwing.
- Malformed `--receipt` token → rejected at the CLI boundary (parse error); the
  Mac composes receipts from typed fields so it can only emit valid tokens.
- A receipt whose reference no longer resolves (deleted branch, migrated id) →
  the chip still renders the token (receipts are stable pointers, not live
  fetches); only `pr:` chips attempt a link.

### Operational boundary

Reads are daemon-less journal folds — no lfd, no network — so the Trajectory
section must never block the plan pane's paint (same invariant as the rest of
the Mac's reads). The write is one `lf memory add` subprocess against the Home
endpoint; a remote Home costs one routed request, already the Home-control cost.
Poll trajectory on the plan pane's existing 30s refresh, not a new timer.

### Exclusions (named for later serial PRs)

- **Calibration approval** — present proposed objective/KR changes with evidence
  and an approve/edit/reject action over `lf pm project update`. Its own slice;
  needs the proposal surface and KR-mutation parity.
- **Beat scheduling** — arrange next-work slots / cadence against existing
  Projects and Tasks (cron / Project selection). Its own slice.
- **`lf receipt show` raw-record drill** — opening the underlying chat turn /
  trace / worker report from a chip. The top-level command does not exist yet;
  PR 1 shows receipts and links only `pr:` references. Building the drill (CLI +
  Mac) is a follow-on that belongs with the Auditability bet's viewer, reused
  not rebuilt here.
- **iOS trajectory parity** — shared DTOs make it additive; sequenced after the
  Mac slice settles.

## Review notes

- No Mac-only domain type: the section decodes `MemoryFact`/`Receipt` and renders;
  it invents no session or planning model.
- CLI parity is structural, not asserted: the Mac calls the same `lf memory`
  verbs the terminal does, so any state the Mac shows is a state `lf memory log`
  shows.
- Every terminal fallback this slice removes (recording trajectory, reading the
  evidence trail) is one the proof-week failure log would otherwise have counted.
