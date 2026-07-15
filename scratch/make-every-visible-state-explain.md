# W2-123 — shared visible-state attention

## User-visible outcome

The person scanning Work in the Mac app or `lf roadmap` sees one shared Task
attention signal and one reason. Green means a live body is advancing without a
human handoff. Red means a human handoff or local recovery is required. Black
means no body is expected and no unsettled local progress exists. Unknown means
the evidence required to choose among those states was unavailable; it never
falls back to black.

PM completion remains an independent fact. A clean unstarted Task and a clean,
settled completed Task can both be black; a completed Task with dirty local work
is red.

## First serial PR

Add a Rust-owned `TaskAttentionSnapshot` to the shared status/roadmap Task DTO.
It carries the derived level, reason, observation time and evidence age, legal
Task controls, and the raw constituent evidence:

- PM completion and optional Session status;
- process observation (`observed`, `not_expected`, `not_applicable`, or
  `unavailable`) and observed liveness;
- next owner and active PR phase already present on the Task row;
- local-progress observation, including dirty changes, Task-authored commits
  ahead of the active PR base, recovery-required intent, and explicit missing or
  unavailable workspace evidence.

Swift decodes this DTO, uses its level to decide whether a Task enters NOW, uses
its controls instead of reconstructing lifecycle legality, and renders its
reason in both visible rows and accessibility labels. The CLI prints the same
level and exact reason.

## Source of truth

- PM completion: cached `PmTaskSummary.completed`.
- Session intent/reason/freshness: durable `TaskSession`.
- Process observability/liveness: the single machine-wide `TmuxLiveness`
  snapshot already used by `lf status` and `lf roadmap`.
- Delivery: durable `TaskPr` phase and immutable active-PR `base_commit`.
- Local progress: the Task-owned worktree at read time, using Git cleanliness
  and `HEAD` relative to the active PR base. A merged or abandoned PR is settled
  delivery, so its historical commits are not local unsettled progress.
- Body Working/Stalled/Recovering categories, write leases, and recovery policy
  remain owned by W2-135. This PR consumes the current liveness boundary and
  does not introduce a parallel heartbeat or watchdog.

## End-to-end proof

One shared fixture contains live advancing, live human wait, dead dirty, dead
authored commits, clean backlog, completed, stale active intent, and unavailable
workspace evidence. Rust round-trips it and independently proves the projection
rules. Swift decodes the same file and proves NOW membership, shared controls,
and accessibility text. CLI rendering tests prove the attention reason, not the
older independently selected next-move reason, reaches the terminal.

Run:

```bash
cargo test -p loopflow lf::commands::waves
cargo clippy -p loopflow -- -D warnings
cd swift && swift test
```

## Affected surfaces and consumers

- Rust `lf status --json` Task detail DTO.
- Rust `lf roadmap [--json]` Task DTO and terminal rendering.
- Shared DTO fixtures under `tests/fixtures/dto/`.
- Swift `WaveTaskWork` and `RoadmapTask` decoders.
- Mac NOW/Roadmap rows, lifecycle buttons, colors, and accessibility labels.
- iOS and Wave Chat do not currently render `RoadmapTask`; strict DTO decoding
  makes the field available without creating a second rule there.

## Absent and error states

- No Task Session: process and workspace are not applicable; clean open backlog
  is black.
- A terminal Session with settled delivery and no worktree: no local recovery is
  expected; it is black.
- A non-terminal Session whose worktree is missing: the absence is observed and
  recovery is red.
- A Git inspection failure: local progress is unavailable. If a live advancing
  body or live human handoff already determines the result, that result holds;
  otherwise attention is unknown with the inspection error.
- No tmux installation while active intent claims a process: process evidence
  is unavailable, controls are withheld, and attention is unknown unless an
  independent live handoff can prove red.
- Empty PM planning remains the existing `Evidence::Unavailable`, never an
  empty Wave.

## Operational boundary

Keep the existing one-tmux-snapshot and zero-network roadmap read. Probe Git
only for Tasks with durable Sessions: one cleanliness check and, only for an
active PR, one `HEAD` read. Do not inspect file contents or run one subprocess
per rendered field. Every failed probe becomes DTO evidence instead of failing
the machine-wide roadmap.

## Exclusions and later serial PRs

- Do not duplicate W2-135 body generations, progress leases, stall deadlines,
  safe recovery, or BodyObservation categories.
- Do not solve Project blocker preservation in this PR; it remains a later
  W2-123 serial slice.
- Do not add Wave, Project, Run, Home, transition-history, Audit, or Wave Chat
  presentation yet. Their shared reason/owner/control coverage follows after
  the Task attention contract lands.
- Dogfood accounting for raw-file drops is project evidence, not a code change
  in this slice.
