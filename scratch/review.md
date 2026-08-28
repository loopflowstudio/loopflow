# Slice review: LOO-227 Project child-resume authority

## Scope

The Task restores the ordinary `lf task resume` path for a Project controller
without treating a generic Run id or optional execution trace as authority.
The controlling Project must publish exact, durable authority before provider
work; retries and controller recovery must preserve the Task while unrelated,
stale, superseded, or missing authority remains denied.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Project pursuit resumes a parked child | A real Project phase uses the existing Task-resume path | Project controller publication issues authority before provider work; `resume_task_async` consumes it through the normal command core | `project_runner_control_resumes_task_across_phase_and_process_recovery`; CLI call graph `task resume` → `resume_task_async` → child launch | Pass |
| Authority is exact | Bind control to Project Work, controller Run, flow position, and Steer frontier | SQLite stores the token hash against Project and Run; authorization resolves the Task's immediate Project and validates its Ready Work, durable flow position, and current Steer sequence | `project_child_control_survives_phase_and_process_recovery_exactly`; store SQL and row mapping | Pass |
| Retry is idempotent | Repeating resume does not create duplicate Task Work or Runs | Repeated resume returns the same existing Task Work; the stable child controller launch is reused | `project_runner_control_resumes_task_across_phase_and_process_recovery` | Pass |
| Stale and unrelated callers fail closed | Another Project, a superseded controller, or a stale phase/direction cannot mutate the Task | Exact parent lookup, token hash, controller Run, flow position, and Steer checks all precede Task mutation and launch | Store fixture covers unrelated Project, new Steer, phase advance, and replacement; command fixture covers stale and superseded controllers | Pass |
| Missing basis fails before pursuit | A Project cannot continue provider work without actionable child-control state | Initial controller publication requires capability issuance; phase advance requires rebinding before sending the phase; errors name the missing durable basis and flow through Project failure recording | `project_child_control_survives_phase_and_process_recovery_exactly`; `project_failure_remains_resumable_in_work_state`; `publish_project_controller` and runner error path | Pass |
| Authority is not bypassed or leaked | Keep local User control while an in-Run caller must present the capability; do not pass Project control into the child | Resume checks at command entry before PR reconciliation and again before launch; provider boundaries inject the Project token while child execution boundaries scrub it | `in_run_task_resume_requires_project_control_before_mutation`; environment scrub tests; negative search of all resume callers and capability writers | Pass |
| Release materialization preserves the fixture | Draft migration canonicalization cannot delete the test's SQL source | Tests resolve the draft by marker rather than compile-time draft path | `uv run pytest python/tests/test_materialize_rust_tests.py -q` (3 passed) and the release-materialized focused Rust fixture | Pass |

## Source review

The effective model is one controller-owned capability row per Project. A
token is presented only in the Project provider environment; SQLite retains
only its hash. Rebinding changes the exact flow/Steer basis, while controller
recovery replaces the holder Run and capability. Generic Runs remain evidence
and provenance rather than mutation authority.

The public surface remains `lf task resume`; there is no special Project
command or compatibility bypass. The CLI reaches `resume_task_async`, which
checks authority before PR reconciliation, and the child-launch boundary
checks it again. Only the Project runner writes capability state. Historical
`agent_turns` references survive solely in migration reconstruction and are no
longer a runtime authority source.

The closest safe production-like demonstration used a temporary configured
repository and planning database plus a stubbed process-launch boundary. It
entered through the production Project controller-publication function and the
ordinary async resume core. Mutating a live parked Task only to manufacture
review evidence was intentionally not attempted.

## Boundary follow-up

The parent-to-child inventory in `scratch/questions.md` does not invalidate
this slice. LOO-227's observable contract is the immediate Project's authority
to resume an existing parked Task. Shared Wave overrides such as `task steer`
are a distinct control boundary: applying only the Project token there would
remove documented Wave behavior without supplying an exact Wave capability.
The branch therefore leaves those commands unchanged rather than creating a
partial authority hierarchy.

`git diff --quiet 45e9db406..HEAD -- rust python scripts tests` returned zero,
proving the inventory pass changed no executable source after the first slice
review. The explicit Project capability remains a coherent endpoint for this
Task; a later Wave-control design can compose above it without reopening the
resume path or treating Run attribution as authority.

## Verification

- `cargo test -p loopflow project_child_control_survives_phase_and_process_recovery_exactly -- --nocapture`
- `cargo test -p loopflow project_runner_control_resumes_task_across_phase_and_process_recovery -- --nocapture`
- `cargo clippy --all-targets -- -D warnings`
- `uv run pytest python/tests/test_materialize_rust_tests.py -q`
- `git diff --check`

These passed on the current tracked and untracked content before this review
artifact was added. The artifact changes no executable input, so the focused
behavior and static-analysis proofs were not rerun.

## Disposition

No blocking findings. The slice matches the Task's authority boundary, restores
the reported resume surface through the existing path, and is ready to publish
for PR review.
