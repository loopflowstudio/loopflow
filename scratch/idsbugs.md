# Readable task identities

Accepted, 2026-10-13. Jack Heart requested `pursue` after reviewing this design.
The accepted scope includes the shared short-ID function, user-scoped branches,
and the preservation and repair boundaries below. Standalone design.

## Accepted outcome

Use one Rust function to present a Task by its known Linear identifier or a
short local hex code, including generated Session names and new user-scoped
branches.

Jack Heart: “I would prefer to use Linear IDs when those are available in
user-facing situations.”

Jack Heart: “there should be one function that gets the short task ID for a
task and understands: if Linear is available use Linear, if not use the
six-character hex code, etc”.

Accepted branch example: `user/eng-123-fix-login` — ID plus a few title words.

## The demo

With a locally known `ENG-123` titled “Fix login”, start Task work. Its generated
Session name begins `ENG-123`; Task labels agree across CLI, Desktop, history,
and agent context. A new branch is `user/eng-123-fix-login`, and its default
workspace segment is `eng-123-fix-login`.

Repeat offline with a local-only Task: display `a1b2c3`, create
`user/a1b2c3-fix-login`. Later Linear synchronization changes the displayed
identifier to `ENG-124`, while the recorded branch, checkout, and PR retain
their identity. Known Linear identifiers remain usable offline.

## Data structures and key functions

- Keep `TaskId`, Linear's opaque issue UUID, and its readable identifier
  distinct. Durable IDs remain keys in SQLite, associations, and JSON ID fields.
- Existing `Task.identifier` and Session `task_identifier` carry the common
  presentation value. Do not add another stored display-ID column or DTO.
- `short_task_id(conn: &Connection, id: &TaskId) ->
  StoreResult<Option<String>>` in the SQLite Task reader. Resolve locally known
  Linear identifier first; otherwise use six lowercase hex characters from
  `TaskId`, extending to the shortest unique prefix on collision. `None` means
  the Task is absent. No network access. Use the same read snapshot as the
  surrounding projection; resolve once per distinct Task in batched listings.
- All presentation paths consume this result, directly or through their Task
  projection. No independent shortening in Swift, prompts, SQL, or branches.
- `plan_task_placement` composes the lowercase short ID with at most three
  title words, capped at 32 title characters and trimmed to a valid segment.
  Delegate user prefixing to existing `WorktreeName`/`plan_branch_placement`.
  Keep explicit workspace names usable; they replace the title portion for a
  generated branch. Separate generated segment validation from the existing
  2–5-word authored-name rule.
- Generated Session titles begin with the short ID and retain their purpose.
  Creation and generated rename suggestions share the formatter. Repair an
  existing generated title's recognized Task-ID token through the existing
  fenced rename writer during owned Session updates/reconnect and local Task
  mapping updates. Preserve purpose and human-assigned titles. Passive reads
  never rename Sessions; native title propagation uses the existing path.

## Implementation and review findings

Implemented in this checkout, 2026-10-13:

- `children.rs::short_task_id` owns Task labels. Task/checkout reads, Session
  summaries and bindings, history, activity, CI reports, automation, and retained
  filing recovery consume it. Batched Session/history readers cache each Task's
  label in their read snapshot. Existing durable selectors remain intact.
- `plan_task_placement` uses the shared Task label and the existing author-aware
  placement primitive. Default title slugs contain at most three words and 32
  characters. Explicit workspace names retain their existing directory segment.
- The Session creation/rename writer adds the Task prefix. Mapping writes repair
  open generated titles; owned attachment and binding updates repair retained
  titles. Human-assigned names and passive readers retain their boundaries.
- Review found activity and retained-filing recovery also bypassed the Task label
  owner; both are cut over. The occupied-branch regression proves a readable name
  cannot adopt another checkout. Native title propagation uses the existing path.
- Two inspected historical Sessions contained UUID placeholders in Task labels,
  not canonical titles; details remain at
  `e3837c9c71:scratch/questions.md`. Synthetic fixtures prove
  generated-title repair without rewriting saved transcript content.
- Compression shares the per-read label cache with CI reports, removes obsolete
  SQL label columns, and reuses the loaded Session during fenced title repair.
  Explicit workspace names now need one placement plan; `WorktreeName` supplies
  the branch's author prefix before the directory segment is selected.

Reconciled, 2026-10-14: source review found no bounded implementation mismatch.
The queue's compression streams collision candidates and computes the longest
shared prefix, then adds one character; it retains the six-character minimum
and includes completed Tasks. Session history and binding readers resolve
cached labels while constructing rows, removing the second mutation pass.
Recorded branches bypass Git-author lookup; new branches retain author prefixing.

Upstream `e3c3a038aa` (offline data clones) merged without conflicts in
`89e116d8c4`. Clone isolation preserves planning identities and Session history;
it does not require another label formatter or change the accepted repair
boundaries. Source review found no reason to change the approach. No Wave is
associated with this work.

## Delete — do not maintain

Completed: removed `TASK_IDENTIFIER` SQL formatting, `derive_workspace_slug`,
the `lf/<UUID>/...` branch generator, and the independent twelve-hex workspace
suffix. Raw stored identifiers remain persistence data; presentation readers
resolve through `short_task_id`. Replaced old naming assertions while preserving
selector ambiguity, placement-layout, and worktree-name coverage. Removed
`task_columns()` allocation and unused label/NULL columns in Task, checkout,
Session and CI projections; the surviving Task SQL is `TASK_COLUMNS`.

## Constraints and forbidden outcomes

No schema migration is expected. Existing database values and accepted durable
selectors continue to work. Local hex selectors must round-trip through commands;
ambiguous selectors must never choose a Task silently. Collision checks include
all stored Tasks, including completed Tasks.

Never rename an existing branch/checkout or rewrite a PR to improve its label.
Persist new placement once; later identifier/title changes do not recompute it.
An unrelated occupied branch/path must not be adopted because its readable name
matches. Preserve current ownership checks and actionable collision errors.

Keep repository layout adaptation. No branch-convention inference, extra naming
configuration, synchronous Linear fetch, Swift-only formatter, or rewrite of
historical identity/transcripts. Do not mistake a local placeholder for a Linear
identifier merely because the Task has a pending export UUID.

## Delivery

Accepted queue boundary, 2026-10-14: Jack Heart requested compress, refresh,
and gate before landing PR #1550. Auto-merge is disabled and the earlier landing
wait was interrupted. Landing belongs to the caller after the queue succeeds.

Implementation and source reconciliation are complete. The current compression
changes remain in the working tree. Gate reviewed the full diff against
`origin/main`, including those changes; no further production repair or product
decision was identified.

Review on 2026-10-14 found two test repairs: the activity revision fixture now
creates a complete Session through the existing helper, and the primary-Session
assertion expects the accepted ID-first title. No production change was needed.
The broad run passed 2,595 Rust tests; both repaired tests then passed a focused
rerun. Architecture, 432 Python tests, 75 website tests, and the headless Desktop
build with 505 Swift tests passed. Standard harness exclusions remain unchanged.

Those earlier checks also passed final formatting and Clippy. Full commands,
resource evidence, and accepted implementation assumptions remain at
`e3837c9c71:scratch/idsbugs.md` and `e3837c9c71:scratch/questions.md`.
The resource preflight issue was resolved with the isolated uv cache; it is
not an outstanding blocker. No resource policy or test selection was weakened.

Check (2026-10-14): `git diff --check` and `lf context` passed; reconciliation changed prose only, with current-tree acceptance owned by the queue's gate.

## Queue gate, 2026-10-14

Review retained the single label owner, per-read caches, generated-title fencing,
and persisted checkout placement. Docs describe the new names and offline behavior.
The upstream native macOS process reader exposed two stale tests that expected a
missing or failing `ps` to make observations unavailable. Their expectations now
distinguish native macOS reads from the other platforms' `ps` reader; production
code is unchanged. The local runner now follows CI's separate clone-test sandbox
boundary, with command-plan coverage for Cargo/nextest and macOS 15/26.

The standard affected-suite run passed architecture, 450 Python tests (plus the
separate checkout-observation test), 75 website tests, the headless Desktop build,
507 Swift tests, and 2,599 of 2,601 Rust tests. The two failures above passed after
repair. The ordinary 18 Rust exclusions remain; Linux and installation proofs
remain owned by their configured CI jobs. No manual display check is required.
Only repairs received focused reruns; the broad run's failed receipt is retained
at `.lf/tmp/gate/run-8654`, rather than rewritten as a passing aggregate.

Resource inventory passed before and after testing (66.8/64.5 GiB free against
the unchanged 32 GiB floor). The first wrapper preflight timed out at 60 seconds
inside build cleanup. A standalone recovery also timed out that optional cleaner
at 120 seconds, reporting no removals; its resource measurement passed. While it
held the cleanup lock, the wrapper's existing nonblocking recovery path admitted
the successful measurements and affected suites. The isolated uv cache was used
throughout; no resource threshold, worker limit, or test exclusion was weakened.

Check: `UV_CACHE_DIR=/tmp/loopflow-idsbugs-uv-cache LF_BIN="$PWD/target/debug/lf" uv run python scripts/test.py --base origin/main --reuse-passing < /dev/null` completed affected suites with only the two subsequently repaired Rust failures described above.
Check: `UV_CACHE_DIR=/tmp/loopflow-idsbugs-uv-cache uv run --no-sync python scripts/test_network.py uv run --no-sync pytest python/tests/test_gate_bounded.py` passed 48 tests after the runner repair.
Check: `UV_CACHE_DIR=/tmp/loopflow-idsbugs-uv-cache TMPDIR=/private/tmp uv run python scripts/materialize_rust_tests.py --prepare-cli -- uv run --no-sync python scripts/test_network.py cargo nextest run -p loopflow --lib -E 'test(unfinished_process_retains_identity_across_terminal_failure_interrupt_and_pid_reuse) | test(snapshots_follow_detached_and_replaced_records_without_capture_files)' --build-jobs 4 --test-threads 4 --no-tests fail < /dev/null` passed both repaired Rust tests.
Check: `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, and `uv run ruff check scripts/test.py python/tests/test_gate_bounded.py` passed after repairs.
Check: `UV_CACHE_DIR=/tmp/loopflow-idsbugs-uv-cache TMPDIR=/private/tmp uv run python scripts/materialize_rust_tests.py --prepare-cli -- cargo test -p loopflow --test integration --jobs 4 data_clones:: -- --test-threads 4 < /dev/null` passed all four clone tests under their own sandbox.
Check: `git diff --check` passed; `gh pr view 1550 --json state,autoMergeRequest` confirmed OPEN with no auto-merge request.

Gate is complete with no unresolved product failures. Compression and gate repairs
remain uncommitted in this checkout. Publication and landing belong to the caller;
this Flow neither landed PR #1550 nor re-enabled auto-merge.
