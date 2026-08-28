# 5 Whys: The Mac read a populated Home with an incompatible `lf`

## The Problem

The configured Dev Mac app kept Wave fleet, roadmap, and Activity reads loading,
then presented zero or empty Work even though the selected local Home contained
five Waves and populated planning.

## Chain

Populated Work appeared empty → Mac `lf` reads failed → Dev launch paired a
selected Home with the wrong helper → bundle location stood in for executable
identity → Mac and Dev tooling duplicated machine-install authority

**Problem**: The Mac showed a zero Wave fleet, no roadmap, and persistent
loading despite a populated selected Home.

**Why 1**: Every relevant `RegistryQuery` subprocess failed before producing a
decodable DTO. With no last-good fleet, the console then treated the absent value
as an empty array and rendered an empty claim beside the failure signal.

↳ *Could we have caught this earlier?* A configured cold-launch proof that
requires a nonzero known fleet and a terminal Activity state would have failed
before the regression reached the desktop. Fixture-only decoding could not.

**Why 2**: LaunchServices receives only explicitly forwarded environment. The
Dev launcher copied legacy `LF_HOME` / `LF_DB_PATH` but dropped the active
`LF_CONTROL_HOME` / `LF_CONTROL_DB_PATH`, sending the helper to an older store.
Preserving those variables was necessary but insufficient because the bundled
validation-only helper was still not compatible with the selected development
store.

↳ *What process allowed this?* The launcher maintained one manual list of Home
routing variables while Swift independently chose an executable. No check
validated the resulting executable/store pair.

**Why 3**: The Mac policy treated the app's bundled `lf` as its exact wire peer
and explicitly rejected PATH fallback. That was safe only if bundle provenance
also guaranteed compatibility with the selected Home. In the Dev app it did
not: the bundle was release-provenance and validation-only, while the Home was
owned by an installed development binary carrying draft migrations.

↳ *What assumption was wrong?* "Built beside this Swift app" was assumed to be
stronger identity than "selected by the Home's machine entry gate." It protects
against an arbitrary checkout binary, but it cannot prove store compatibility.

**Why 4**: Production activation and Dev assembly evolved different meanings for
the same helper path. Production rewrites the installed app helper to the
machine entry gate; Dev assembly embeds a standalone validation helper. Unit
tests reinforced the bundle-only policy, and the error UI fixture kept Waves
available even when every other reading was unavailable. Neither test crossed
Dev launch → executable selection → selected Home → visible terminal state.

↳ *Why was that assumption encoded?* Two valid safety mechanisms were designed
locally: validation-only helpers prevent a Dev UI from migrating a release Home,
and installed development Homes pin draft-compatible executables. Their
composition had no owner, so each layer selected half of the pair.

**Why 5 (Root)**: Executable/store compatibility is durable machine-install
authority, but the Mac app and Dev launcher duplicated it as bundle-path and
environment conventions. Because no single contract supplied both the selected
Home and its compatible entry gate, helper provenance, Home routing, tests, and
UI truthfulness could drift independently.

### Parallel rendering branch

The read failure became a plausible empty state because views consumed
`PodiumReading.value` as an optional collection instead of rendering the enum
exhaustively. The model preserved loading, available, unavailable, last-good,
and reason; the presentation layer collapsed those facts back into `nil` and
`[]`. The fixture repeated that collapse by making the fleet available during
its error state. This did not cause the subprocess failure, but it hid its
consequence and made the broken surface look healthy.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 4 | Should Dev assembly make `Contents/MacOS/lf` the machine gate, or should the Mac consume an explicit install-owned authority receipt? | High |
| Why 5 | What exact helper/store contract should offline app operation use when no machine installation exists? | High |
| Rendering branch | Which other `PodiumReading` consumers still infer empty state through `value ?? []` instead of switching exhaustively? | High |
| Why 1 | Why does current `lf activity` JSON omit the Swift-required `run_id` on some items? | High |
| Proof branch | Why did the legacy nanoseconds sleep return early for long live-capture delays on this host? | Medium |
| Machine state | Why was a July Xcode-built app still running without a usable helper, and can stale app provenance be named in-product? | Low |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Resolve the fixed OS-account machine gate before the offline bundle, and render unavailable fleet/Activity states without empty copy. | This reported instance and its misleading presentation |
| Structural | Give Dev launch and the Mac one install-owned local-control authority containing the selected Home and exact entry gate; remove independent environment allowlists and executable search rules. | Any helper/store provenance mismatch across release and development Homes |
| Structural | Render `PodiumReading` exhaustively through one shared state boundary and make fixtures vary fleet, roadmap, and Activity failure independently. | Unavailable or unknown evidence silently becoming healthy empty state |
| Systemic | Gate the installed Dev app with a disposable populated development Home whose schema is intentionally ahead of the validation-only bundle, and require nonzero fleet/roadmap plus terminal failure presentation. | Future feature combinations that unit and DTO fixture tests cannot see |

## Changes to Implement

- [x] Use the install-owned local-control authority: the fixed machine gate
  reads the active receipt that binds the exact executable and store; the
  bundled offline path applies only when no installation exists.
- [x] Make Dev launch and Mac process launch consume that authority, then
  delete the manual `LF_*` forwarding list and arbitrary PATH scan.
- [ ] Add an installed-Dev-app integration proof using a disposable populated
  development Home newer than the validation-only bundled helper.
- [ ] Audit every `PodiumReading` consumer and replace optional/empty inference
  with exhaustive loading, available, last-good, and unavailable rendering.
- [ ] Track the missing Activity `run_id` wire mismatch separately; keep its
  current explicit unavailable state until the shared DTO contract is repaired.

With the install-owned authority boundary established, the next prevention is
the installed-Dev-app integration gate. It should create a disposable populated
development Home newer than the validation-only helper and require the receipt-
selected gate to load it without any `LF_*` routing environment.

## Restoration Record

### Finish line

The configured Dev Mac app reads the same selected control Home as the `lf`
process that launched it. A cold launch shows the populated Wave roster and
roadmap, Activity leaves its loading state, and a failed first read shows the
subprocess reason without also claiming that the fleet is empty.

Proof must exercise the Mac app's production `RegistryQuery` through the real
launch environment and selected machine entry gate. Fixture-only Swift decoding
and a healthy shell `lf` do not count.

### Observations

- The selected control store is
  `~/.lf-dev/installed/local-04115a69e0c34b198bf110976b32f390/loopflow.db`.
  `lf ls --all --json` returns five Loopflow Waves; `lf roadmap --all --json`
  returns populated product planning.
- The Dev app bundles a validation-only release-provenance `lf`. Before the
  restore, the LaunchServices environment produced by `scripts/loopflow-dev.py`
  fell back to `~/.lf/loopflow.db` because `_app_environment` forwarded only
  `LF_HOME` and `LF_DB_PATH`, not the selected `LF_CONTROL_HOME` and
  `LF_CONTROL_DB_PATH`.
- Against that fallback store, the bundled helper reproduces both reported
  failures: `ls --all --json` and `roadmap --all --json` exit nonzero with
  `no such column: retired_at`. The fallback database is at migration
  `0.12.12.001_release`; Wave retirement arrives in `0.12.13.001_release`.
- After forwarding the selected Home, the rebuilt bundled helper reaches the
  intended database but still cannot read it. The Home is owned by the active
  installed development `lf` and carries its draft frontier; the bundled
  release-provenance reader reports incompatible schema and then fails on
  `work_state` / `work_kind`. The machine entry gate at `~/.local/bin/lf`
  successfully resolves the matching active binary and reads this store.
- `LF_CONTROL_BIN` is not usable as the app reader on this machine: it points
  to a stale pre-command `~/.lf/bin/lf`. The machine entry gate, not this legacy
  inherited path, is the current install authority.
- The primary Podium model preserved read failures and their reasons. Before the
  restore, its console nevertheless rendered `No Waves found.` whenever the
  failed fleet read had no last-good value, contradicting the adjacent
  unavailable signal.
- A stale Xcode-built Loopflow process from 2026-07-21 is also running from
  DerivedData and has no bundled `lf`. It is evidence of the reported machine
  state, but changing or deleting that build is not required to restore the
  separately identified Dev app path.

### Rejected hypothesis

Preserving the complete selected-Home environment across `open --env` will make
the bundled helper read the populated control store it was launched to operate.
The first half holds; the second fails because a release-provenance helper is
not the binary that owns an installed development store.

### Restored model

Resolve the fixed OS-account gate at `~/.lf-machine/install/gates/1/lf`. Its
active install receipt selects the compatible executable and store together,
regardless of PATH or inherited Home variables. Keep the validation-only
bundled helper as an explicit no-install fallback. Restrict fleet and
Activity empty copy to successful reads.

### Restoration proof

- `uv run python scripts/loopflow-dev.py run` rebuilt, signed, and launched
  `~/Applications/Loopflow Dev.app` with only the development repository
  override; the machine install receipt supplied the active Home.
- The installed-bundle capture at `/tmp/loo-274-gate-retry.png` ran
  `~/Applications/Loopflow Dev.app/Contents/MacOS/Loopflow` with every `LF_*`
  routing variable removed and `PATH=/usr/bin:/bin`. After a 40-second settle it
  showed the five Loopflow Waves and populated Work rows. The exact machine gate
  under the same hostile environment reported five Loopflow Waves; roadmap
  reported product with three Projects and 88 Tasks. The capture checksum is
  `3e7aab97d12f5ed1aa3bb0635d6e88eeb408fe896863a0f00938b3ac237e5130`.
- Activity left `Reading Activity…`. Its current wire mismatch surfaced as
  `Activity unavailable` with the exact missing-`run_id` decode reason, and did
  not render `No Activity in this window`. This distinguishes restoration from
  the reported indefinitely loading or plausibly empty surface.
- A gate capture first caught Activity back in `Reading Activity…`: each
  same-scope retry reset an unavailable reading to loading before the command
  finished. Retries now preserve the current terminal reading until new evidence
  arrives; the second 40-second capture above remained explicitly unavailable.
- `/usr/bin/time -p lf activity --since 7d --limit 50 --json` returned 50 items
  in 6.67 seconds. The command completes with populated JSON; the visible Mac
  failure is a named DTO mismatch rather than an empty ledger.
- Focused proof: `uv run pytest python/tests/test_loopflow_dev.py` passed, and
  `swift test --package-path swift --filter LocalWaveAgentLauncherTests` passed
  14 authority and launcher behaviors. The earlier restoration pass also
  compiled and signed the app plus `LoopflowUITests`; gate owns rerunning that
  broader build against the final branch content.

### Review

- Reads and controls share `controlLfPath()`; there is no second Mac query or
  Swift-owned authority type. The function selects either the receipt-backed
  machine gate or the validation-only offline fallback. It never scans PATH,
  and a receipt with a missing or non-executable gate fails rather than falling
  through to another store.
- No database migration or user-state edit was needed. The fallback and active
  stores remain intact, and the stale Xcode process was left untouched.
- Capture targeting changes only automated capture mode. Ordinary launches
  still restore the user's repo-scoped Sessions surface.
- The missing `run_id` Activity DTO mismatch remains deliberately visible and
  outside this restore. Repairing that wire contract is separate follow-up,
  not a reason to hide the fleet and roadmap recovery.

### Near-misses

- Migrating, rebuilding, or discarding the fallback `~/.lf` database.
- Using the bundled release-provenance helper as the peer for a draft
  development store.
- Treating authored `wave/*/GOAL.md` placeholders as proof that registry reads
  recovered.
- Showing a zero/empty fleet alongside an unavailable read.
- Capturing before the live reads settle. The legacy nanoseconds sleep returned
  early for long delays here; the duration-based sleep now holds the declared
  capture interval and returns on cancellation instead of taking a false frame.
- Using obsolete capture variable names. The current knobs are
  `LOOPFLOW_UI_TEST_DELAY`, `LOOPFLOW_UI_TEST_WIDTH`, and
  `LOOPFLOW_UI_TEST_HEIGHT`; `..._SNAPSHOT_*` names silently use the 2.5-second
  default and capture the healthy live query while it is still loading.

## Slice Review — 2026-08-27

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Installed authority | The configured app reads the Home selected by the machine installation, independent of inherited routing variables and PATH. | Reads and controls both resolve `~/.lf-machine/install/gates/1/lf`; only a machine with no install receipt uses the bundled validation helper. | Exact installed app launched with all `LF_*` routing removed and `PATH=/usr/bin:/bin`; launcher suite passed 14 authority/control behaviors. | pass |
| Populated fleet | A cold launch exposes the real local Wave roster. | `RegistryQuery` receives `lf ls --all --json` through the machine gate. | `/tmp/loo-274-review-settled.png` shows **5 Waves**; the exact gate independently returned the same five Loopflow Waves. | pass |
| Populated roadmap | Roadmap loading terminates with the selected store's real Work. | `RegistryQuery` receives `lf roadmap --all --json` through the same gate. | The installed-app capture shows populated Work rows; the exact gate returned product with 3 Projects and 88 Tasks. | pass |
| Terminal Activity state | Activity must leave `Reading Activity…` even when its DTO cannot decode. | A failed first read becomes `Activity unavailable` with the subprocess/decode reason. | The installed-app capture shows the current missing-`run_id` reason and the terminal unavailable state. | pass |
| Activity retry stability | A periodic retry must not hide the last terminal Activity state while its subprocess is running. | Same-scope refreshes retain the existing reading; a scope change alone resets to loading. | `PodiumModelTests.unavailableActivityStaysVisibleDuringRetry` holds a retry open and observes the prior failure; the 40-second live capture remains unavailable across multiple retries. | pass |
| Failed reads are not empty | Unavailable fleet, roadmap, or Activity evidence must not claim healthy emptiness. | Empty copy is restricted to successful empty readings; the error fixture makes every affected reading unavailable. | Live capture shows no Activity empty claim; `PodiumStateTests.testUnavailableIsNotRenderedAsEmpty` pins all three surfaces. | pass |

The exact-head capture checksum is
`3e7aab97d12f5ed1aa3bb0635d6e88eeb408fe896863a0f00938b3ac237e5130`.
Focused review proofs passed: `uv run pytest python/tests/test_loopflow_dev.py`,
`swift test --package-path swift --filter LocalWaveAgentLauncherTests`, and
`swift test --package-path swift --filter PodiumModelTests` (14 tests in the
final focused filter). Before the bounded retry fix, gate also passed 205 Python
tests and the 230-test full Swift suite; the final tree passed the Swift
platform-boundary check, Mac package build, eight distinct render proofs, and
the signed macOS `build-for-testing` check.

Disposition: the restoration slice is coherent and its applicable Done When
claims hold through the configured installed app. The Activity `run_id` wire
repair, an unversioned install-authority API, the disposable newer-Home gate,
and the broader `PodiumReading` audit remain explicit prevention work rather
than hidden conditions of this repair.
