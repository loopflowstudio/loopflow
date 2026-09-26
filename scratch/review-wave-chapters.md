# Implementation review — Wave chapters

Subsequent human clarification changes the finish line: Wave objective;
Project-owned Tasks, KRs, and metric targets, all presented through the Wave.
The proofs below cover the earlier implementation. Metric target ownership
still needs correction; see `scratch/projects.md` and `scratch/questions.md`.

The public hierarchy is Wave → Task. One internal Project supplies the current
chapter plan; `wave_chapters` owns its binding and resumable transition receipt.
Provider content remains authoritative. Rotation preserves Task identity and
execution state, cancels untouched backlog, and freezes dated prior evidence.

## Findings fixed

- A local Project creation fallback in Task preparation duplicated the chapter
  writer. Removed it; preparation resolves the existing current binding.
- The obsolete Project-promotion launcher remained after its command and skill
  were deleted. Removed the unreachable workflow and its tests.
- A stale Task save could restore its old parent. Ordinary Task updates no
  longer write membership; chapter transfer owns that field.
- Flow reset could erase evidence of earlier execution. First execution now
  persists as a Task event; generic Run evidence is read from Home records.
- Current and historical references used provider IDs, provider slugs, and local
  Work IDs. The shared chapter reads expose exact source identities so Desktop
  links resolve without guessing names.
- Metrics obscured objectives and Tasks in the older workspace. Put chapter
  planning and the full Task list before metrics in both workspaces.
- Remaining skills and docs still taught a Project portfolio/operator tier.
  Replaced those instructions and removed obsolete command examples.

## Proof

- Eight focused Rust chapter checks: classifier, cutover uniqueness, retirement
  versus first claim, identity/Flow/PR preservation, stale saves, CLI grammar,
  registered chapter skills, and the local HTTP-provider lifecycle simulation.
- The provider simulation covers initial provisioning, read-only preview,
  Wave-only Task filing, unknown-state refusal before successor creation,
  started work after preview, lost create/move/cancel replies, retry idempotence,
  fresh chapter content, and historical evidence after later completion.
- Seven Rust wire fixture checks and four Work-binding checks passed. Focused
  Task restart and remaining Task preparation prerequisite checks passed.
- Swift projection, navigation, query, session, and DTO checks passed: 97 checks
  in the cross-surface pass, followed by 28 focused checks after adding exact
  chapter source-reference navigation.
- Native offline captures cover both workspaces at narrow and wide sizes;
  state captures distinguish populated, loading, unavailable, and empty.
  Selected captures are under `scratch/evidence/`.
- CLI help exposes `wave new-chapter/history/update-plan`, and Task creation
  takes only a Wave. Current docs/skills contain no removed Project commands.

## Limits

Provider lifecycle evidence uses a local HTTP simulation. No live Linear
rotation or migration was applied. Native captures use fixture data; the
unbundled development executable lacks the installed `lf` helper, so those
captures do not prove live controls or native click-through interactions.
The hosted matrix, installed application demo, and real accepted chapter
application remain release/gate evidence, not claimed here.

The final failure-path review found that an error preparing the successor could
be hidden by the still-current predecessor. Wave reads now preserve the current
plan while exposing the pending chapter ID, phase, and error. History labels
prepared successors separately from the current chapter. A focused Rust proof
checks the failed-preparation projection; the updated Swift view builds.

The failed-preparation projection check passed, as did 11 historical promotion
and storage compatibility checks after removing the obsolete promotion writer.
These historical reads remain supported without retaining the deleted workflow.

Final `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
The rebuilt CLI help check passed: `lf wave` exposes serve/new-chapter/history/
update-plan, and `lf task start` takes a Wave and describes the chapter Flow
recommendation without requiring a Project selector.
