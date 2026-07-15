# Stable Wave surface

## Outcome

Selecting a repository and Wave opens one calm workspace:

```text
┌─ Wave navigation ─┬─ Objective + Projects ─┬─ Chat ────────────────┐
│ repository picker │ canonical first sentence│ durable Wave thread   │
│ stable Wave rows  │ open Tasks + KR proof  │ composer              │
│ shared lenses     │                        │                       │
└───────────────────┴────────────────────────┴───────────────────────┘
```

The repository is a scope control, not a permanent column. Wave rows keep a
stable position as execution changes. The selected Wave leads with one complete
sentence from its canonical Objective, then every Project's open-Task count and
KR list. Chat remains the default third pane and starts independently of plan or
status refreshes.

No primary element says “registered,” prints a sync timestamp, exposes a raw
subprocess/database error, or substitutes runtime detail for the Wave's meaning.

## End-to-end proof

Use the real Product repository and the deterministic surface fixtures for one
cross-boundary scenario:

1. `lf ls --json` supplies every Wave in Product; the repository picker scopes
   the one stable list without status-based reordering or horizontal scrolling.
2. Selecting Product mounts Chat immediately while `lf status product --json`
   supplies its canonical Objective, Projects, Tasks, and shared lens readings.
   The center pane shows the first complete Objective sentence, every Project,
   each open-Task count, and every KR with proof state.
3. A later status failure preserves that reading. The primary surface says only
   that live detail could not refresh; technical text is available behind help
   or disclosure and does not replace the Objective or Projects.
4. The selected fixture renders at 900×700 and 1440×900. Every Wave remains
   selectable, Wave names wrap rather than clip horizontally, Objective and KR
   text wrap vertically, and Chat retains a usable composer.
5. VoiceOver reads each lens as its shared semantic state and reason, never as a
   color alone.

The maintained fixture matrix covers:

| Scenario | Required reading |
| --- | --- |
| Empty | Repository picker and a restrained “No Waves yet” row; no full-pane empty illustration. |
| Loading | Existing rows stay visible; first load uses a small inline progress affordance. |
| Error | Last-known rows/detail remain; one calm refresh notice replaces raw implementation text. |
| Selected | Product Objective, all Projects, open-Task counts, KR proof, and usable Chat. |
| Future child | The same row and lens render at indentation level 1 without enabling collapse or inventing ownership. |

Proof commands:

```bash
swift test --package-path swift
uv run python scripts/generate_screenshots.py --ui-test-only --direction stable-wave
uv run python scripts/test.py --loopflow
```

The screenshot manifest captures all five states and captures the selected state
at both narrow and wide sizes. An XCUI assertion verifies the lens accessibility
label contains the fixture reason. The signed build-for-testing gate is the
headless fallback when hosted UI execution is unavailable.

## Sources of truth

| User concept | Authority | Mac projection |
| --- | --- | --- |
| Repository scope | `PortfolioService` plus the existing origin-repo scan | One sticky menu selection above the Wave list; `All Repositories` remains a scope, not a rail. |
| Wave identity and ancestry | `lf ls --json` `WaveSnapshot` | A flat, stable list. Rows accept an indentation level and retain `parent_wave_id`, but this Task does not derive a recursive outline or collapse behavior. |
| Objective | `wave/<wave>/GOAL.md` as returned in `WaveSnapshot.goal` / `WaveDetailSnapshot.wave.goal` | The first canonical sentence is a deterministic excerpt, allowed to wrap. The full canonical Objective is disclosed when more text exists; no generated summary is stored. |
| Projects, KRs, Tasks | Linear's cached SQLite projection, joined by `lf status <wave> --json` | `WaveDetailSnapshot.projects` is the selected Wave reading. Open-Task count is `tasks` where `completed == false`; KRs preserve the shared `holds` value. |
| Offline plan fallback | `lf pm show --wave <wave> --json --no-sync` | `WavePlan` also derives open-Task counts from `items[].project` and `completed`, so the authored/cached fallback does not silently lose the Project's strongest quantity. It remains presentation state, not a second wire DTO. |
| Operational lens | The shared green/red/black projection and reason owned by W2-123 | One reusable glass-lens view renders the supplied state and reason for Wave rows and Project cards where present. Swift never derives color from `WaveStatus`, process liveness, next owner, PM completion, or filesystem state. |
| Conversation | The selected Wave listener's durable journal through `WaveChatView` | Chat is instantiated with selection and does not await `lf status`, PM, trace, or supervision reads. |

`WaveDetailSnapshot` remains the one selected-Wave status contract. Project cards
are computed views over it; no second stored “surface snapshot” may discard wire
fields. The authored `WavePlan` is only the explicit fallback for a Wave with no
successful status reading.

## Surface contract

### Navigation

- Remove the 100-point repository rail. Put a menu-style repository picker,
  New Wave action, and refresh/loading affordance in the burgundy Wave column.
- Preserve the current sticky repository selection and `All Repositories`
  behavior, deep-link repo selection, create-Wave target, and origin/worktree
  normalization.
- Sort Wave rows by a state-independent key: repository then localized Wave
  name. Starting, stopping, pausing, or changing lens color cannot move a row.
- Remove objective previews and status pills from rows. A row contains the
  restrained lens and the full Wave name, with a reserved indentation inset for
  future real ancestry.
- Do not query per row. The list consumes the existing machine-wide `lf ls`
  snapshot and keeps the last successful rows on refresh failure.

### Objective and Projects

- Replace the unselected `RoadmapView` in the daily Wave workspace with a small
  “Choose a Wave” state. Roadmap remains a separate product surface; its sync
  timestamp and registry diagnostics do not occupy this hierarchy.
- Keep the center pane persistently visible beside Chat. It scrolls vertically,
  never horizontally.
- Render one complete canonical Objective sentence. Do not line-limit or clip
  it; disclose any remaining canonical text without generating a rewrite.
- Render every Project card with name, open-Task count, and full KR list as its
  strongest hierarchy. Definition prose is secondary disclosure.
- Remove Task rows, runtime status strings, provider names, directives, PR
  receipts, “Next” diagnostics, and the selected-work inspector from the
  default Project stack. This Task does not replace them with a kanban or
  supervision view.
- A Project may show the shared operational lens when W2-123 supplies one, but
  open-Task count and KRs remain more prominent.

### Chat and lens behavior

- Keep `WaveChatView` as the default third pane and preserve its existing journal,
  composer, start/stop, streaming, and child-activity contracts. Durable load,
  failure rollup, and typed references remain W2-174.
- Mount Chat in the same render that accepts a Wave selection. Status refresh
  may update the center pane later and may fail without replacing Chat.
- The lens is a small recessed glass circle with a restrained inner glow,
  specular highlight, and reflective black state. It does not pulse and has no
  eye-shaped housing or status pill.
- Accessibility names the semantic state plus the shared reason. The decorative
  glass layers are hidden from accessibility.
- W2-123 is a hard semantic dependency. Until its typed state and reason land,
  this Task may build the lens presentation and fixtures but must not ship a
  green/red/black fallback inferred in Swift.

## Absent and error states

| Boundary | Meaning and presentation |
| --- | --- |
| No repositories | The picker says no repositories are available and the existing Open Repo flow remains reachable. |
| Repository has no Waves | Show one compact “No Waves yet” row and New Wave action. Do not say “no registered Waves.” |
| First Wave-list read is loading | Show an inline progress affordance without taking over the detail area. |
| Later Wave-list read fails | Keep the last successful/authored list and surface a compact refresh notice. Never replace known rows with zero. |
| No Wave selected | Show a restrained “Choose a Wave” prompt; do not show Roadmap, timestamps, or registry repair text. |
| Authored Wave has no registry row | Objective and cached PM plan remain valid. Live lens/status is absent, not synthesized; Chat keeps its existing start affordance. |
| First detail read fails | Render the authored/cached Objective and Projects when available. Say “Live details unavailable”; hide the raw cause behind help/disclosure. |
| Later detail read fails | Keep the last successful `WaveDetailSnapshot`, counts, and KRs; show the same compact notice until recovery. |
| Objective is empty | Show “No objective written yet” at body scale, not as a giant empty state. |
| Projects are empty | Show “No Projects yet” below the Objective. Do not say “No live projects.” |
| Shared lens absent | Do not infer or label a color. Preserve row selection and spacing with no semantic placeholder. |
| Chat unavailable | Preserve `WaveChatView`'s existing actionable start/retry state; plan rendering remains independent. |

## Operational boundary

- First paint and Wave-list refresh remain daemonless and use one machine-wide
  `lf ls --json` read per cadence, distributed to repositories.
- Selecting a Wave creates no new global or per-row read. It mounts Chat and one
  cancellation-aware `lf status` refresh task; stale completions cannot replace
  a newer selection.
- PM fallback stays cache-only (`--no-sync`) and never reaches Linear from the
  render path.
- Chat load/send does not wait for status, PM, trace capture, screenshots, or
  any future Control surface.
- Retain the current polling cadences. This Task changes hierarchy and recovery,
  not refresh frequency.
- The 900×700 fixture is the minimum supported proof size. Split panes may
  resize, but navigation and Objective/Projects keep usable minimum widths and
  Chat keeps a usable composer without horizontal scrolling.

## Affected consumers

- `WavesView`: repository picker, stable ordering, last-known list states,
  selected/unselected composition, and narrow-width layout.
- `WaveRow` / `WaveViewModel`: remove prose/status pills, preserve ancestry and
  shared lens input, and expose the semantic accessibility reason.
- `WaveDetailPane`: canonical one-sentence Objective, Project count/KR hierarchy,
  generic refresh presentation, and independent Chat mounting.
- `RegistryQuery.plan` / `WavePlan`: decode cached PM items solely to derive open
  Task counts for the explicit fallback.
- Shared DTO fixtures: consume W2-123's required lens state/reason in lockstep
  with Rust and other clients; W2-178 adds no competing wire field.
- `AppTestMode`, `ScreenshotPipelineTests`, and `scripts/screenshots.yaml`:
  deterministic surface states, two desktop widths, and accessibility proof.
- Existing repo deep links, create-Wave flow, listener/Chat protocol, CLI output,
  iOS, and Roadmap semantics remain compatible.

## Ordered serial delivery

1. **PR 1 — stable detail reading (open as #932).** Keep the complete
   `WaveDetailSnapshot`, derive `workMap`, retain the last successful selected
   detail, and prove decode/recovery. This is the read-contract foundation and
   does not complete W2-178.
2. **PR 2 — stable workspace.** After PR 1 lands, rotate this Task's serial
   branch. Fold repository selection into the Wave column, stabilize ordering,
   reshape Objective/Project hierarchy, derive fallback open counts, replace the
   default Roadmap, and add the deterministic screenshot/accessibility matrix.
   Rebase/integrate W2-123's shared lens contract before marking this PR or Task
   complete; do not open a parallel Task worktree or a Swift-semantic stopgap.

## Exclusions

- Active Sessions, Control/Supervision, Run History, kanban, attention ranking,
  and external launch adapters.
- Agent-launched interactive handoff lifecycle, attach descriptors, presentation
  preference, or any Swift-owned Session model.
- Durable Chat caching, failure aggregation, typed reference navigation, and
  popovers owned by W2-174.
- Defining green/red/black semantics, unsettled-local-progress rules, or a Swift
  fallback owned by W2-123.
- Recursive Projects, arbitrary folders, Wave collapse controls, or invented
  parentage.
- New Rust endpoints, PM mutations, polling cadence changes, and iOS layout.

## Done when

W2-178 holds only when the real Product registry and the deterministic fixture
matrix prove the Outcome at both desktop widths, the VoiceOver reason test
passes, all Swift and Loopflow gates pass, and W2-123's shared lens—not a Swift
inference—drives every lens shown by this surface.
