# Attempt history review and Session cutover requirement

2026-09-26 · LOO-298 · Review of `762e546673bf926e469e463fde6bf83c23301950`
plus the cursor-decoder repair described below. The supplied compression note
was preserved in `59046135c` before edits. This review chooses no Flow edge.

## Disposition

The Task attempt foundation passes its focused storage and driver proofs after
one reproduced recovery regression was fixed. **The architectural cutover does
not pass.** `lf session list` still combines four sources; lookup, naming and
attribution retain competing owners. No intermediate publication is warranted.

[The complete design](data-model-one-table-per.md),
[amended approval](data-model-review-feedback.md),
[Session ownership](session-runs-and-current-run.md), and
[dated decisions](questions.md) govern. Session owns Runs and a current Run;
Invocations may be taskless; Task assignment is write-once and Started records
first assignment time. The Task directive's original Session-as-Run-child model
and the earlier MIN(created_at) instructions are superseded.

Jack Heart's comment `50cf5b12-8a17-4390-a6a1-be69a50cbb7f` changes the next
implementation priority: complete the Session read-path cutover and delete the
replaced paths in that same pass. Another owner-only foundation is insufficient.
This review repairs a bounded regression; it does not claim that cutover.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Headless attempts at one position | Ordered attempts, one current Run, only a successful current Run can settle | Task binding inserts through the shared Run constructor; selection, decisions and settlement check the current Run | 20-test durable suite; 29-test Task controller suite, including original-successful-Run recovery | Pass for Task fixtures; taskless driver and public history remain gaps |
| Human attempts and stable conversation | Replacement retains Session, title, feedback and prior Runs; stale actions reject | SQL Task review reservation keeps the Session and appends Run attempts | Durable replacement/current-attempt tests; historical regression below | Pass for SQL Task reviews |
| Historical cursor recovery | Supported captures survive replacing a published attempt | Attempt location previously decoded historical progress as a new cursor; now shares the existing cursor decoder | Failing `historical-review-before.log`, passing `historical-review-after.log`, canonical repeat | Reproduced and repaired |
| Complete ancestry/structure validation | Nullable Task equality, parent/graph/current-pointer constraints | Task constructor and current-attempt checks implemented; runtime loop-child ownership and full taskless path absent | Durable matrix/race tests; existing schema proof; source | Partial; whole-design obligation 1 remains |
| One CLI/usage/bind reader | All kinds use SQL; confirmed bind and Started agree everywhere | No general bind; list/lookup/usage still use old sources; `position_runs` has only test consumers | Source dependency map below; prior canonical CLI receipts retain their limited scope | Gap, obligation 2 |
| Execution preservation | Common Task/taskless driver, captured runtime nesting, Ask retry, feedback once | Task driver preserves its fences; taskless Flow and Ask still have file owners | 29 controller tests use simulated providers; flow_run/flow_session source | Partial, obligation 3 |
| Repository Chapter operation | One common Chapter with frozen predecessor evidence | Per-Wave `wave_chapters` remains | Architecture inventory 32/33; Chapter source | Gap, obligation 4 |
| Populated filesystem import | Preserve all kinds, attempts, outcomes, missingness and timestamps before activation | Six drafts preserve their SQL input; filesystem import and exact attempt mapping unfinished | Prior five canonical preservation tests; final decoder regression on same canonical schema | Partial, obligation 5 |
| DTO/desktop | Typed parents and attempt history, cached grouping, retained pane | Current DTO/Swift consumers unchanged by this slice; no public position history | Source; no mounted/native proof in this review | Gap, obligation 6 |
| Configured acceptance | Backed-up exact Homes converted and read through matching CLI/app | No real Home migration or activation performed | Local isolated stores only | Gap, obligation 7 |
| Deletion and consistency | Remove all alternate owners and match implemented docs | Old Session paths reachable; target docs lead implementation | Searches and metrics below; architecture gap | Fail for cutover, obligation 8 |

## Reproduced recovery failure and repair

`store/sqlite/runs.rs::location_in` deserialized every non-null `review_json`
as `ExecutionCursor`. The supported historical progress forms have no `index`:
`decode_flow_position` instead reconstructs their cursor from the retained root
columns. Consequently a historical review could be read successfully, then fail
when reserving its replacement with `missing field index`.

The regression exercises both historical progress encodings, replacing a Run at
iteration 3. Before the repair it failed at reservation. The repair extracts the
existing decoder into `durable::decode_flow_cursor` and uses it in both readers;
it does not introduce another historical representation or rewrite capture bytes.
The test checks the cursor, node, tuple, ordered attempt number, stable Session,
name, feedback, two retained Runs and byte-identical historical JSON.

No schema, DTO, process-control or publication behavior changed in this repair.
Existing blocker decoding and current root validation remain in the shared
function. The full durable suite includes legacy blockers and decision recovery.

## Executed proof and limits

Receipts are retained under `.lf/tmp/loo298-attempts/` in this checkout.

| Receipt | Command or boundary | Result |
| --- | --- | --- |
| `review-controller.log` | `cargo test -p loopflow --lib controller::task::planning_tests -- --test-threads=1` | 29 pass, 121.8 s including compile; before the decoder repair |
| `historical-review-before.log` | `cargo test -p loopflow --lib historical_review_cursor_can_reserve_a_replacement_attempt -- --test-threads=1` | Fails: missing field `index`; 38.2 s |
| `historical-review-after.log` | `cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests -- --test-threads=1` | 20 pass, 80.2 s including compile; final source |
| `canonical-historical-review.log` | Same historical regression on retained materialized `0.12.23.001_release` source copy | 1 pass, 36.3 s including compile |

Formatting (`cargo fmt --all --check`), all-target Clippy
(`cargo clippy --all-targets -- -D warnings`, 18.5 s) and working-diff whitespace
pass. The architecture checker still fails at 32/33 SQLite owners, missing
`wave_chapters`; its seven other inventories pass. Migration SQL is unchanged,
so previous migration-validation receipts retain their scope. Historical
branch-range whitespace remains unresolved; no draft checksum was rewritten.

There are **49 distinct passing tests** in the two source suites, with the
historical regression repeated on canonical bytes. This is not a full gate.
The controller suite precedes only the shared-decoder repair; the final durable
suite directly exercises that repair and both historical encodings.

The retained canonical copy was prepared by the preceding implementation from
verified source, with six drafts materialized. Only the two repaired Rust files
were recopied for this review; migrations were unchanged. The preceding source
hash receipt still matches 434 of 436 Rust/Cargo inputs; the two differences are
those reviewed files. The earlier five populated canonical preservation tests
retain their recorded scope. The new repeat proves the decoder against the
canonical schema, not a filesystem import or installed upgrade.

Tests clear inherited execution authority, use four low-priority build workers,
serial test execution and a 900-second process-group bound. Resource preflight
passed (97.6 GiB free; this checkout 4.0/12 GiB); no timeout or cleanup fired.
Providers are simulated. No configured provider, real Home, desktop, PR, Task
disposition or Flow navigation changed.

The prior concurrent fresh-Home `usage --json` failure (`no such table:
run_events`) remains an unresolved initialization race. Sequential and separate
pristine-Home passes do not disprove it; see [questions](questions.md).

## Negative architectural proof and exact dependencies

The dependencies below are reachable production paths, not leftover names in
fixtures. They prevent claiming a completed Session cutover.

| Old authority | Reachable dependency | Required replacement before deletion |
| --- | --- | --- |
| Four-source list, `ops/human_session.rs:575` | SQL Task reviews + Ask files + standalone Flow files + interactive manifest scan | Record/import all three Session kinds, then query Sessions joined to their current Runs through the shared decoder |
| Multi-source lookup, `human_session.rs:589` | SQL ownership, standalone token parsing, boundary lookup, manifest/prefix resolution, Ask record lookup | One Session/Run lookup with stable Session ID and exact selected-Run action fence |
| Ask files, `human_session.rs:68` | `human-sessions` owns request, keyed retry, readiness/completion and Run binding | Move request/retry/result and current Run into Session transactions; preserve completed answers and launch-lock/teardown ordering |
| Taskless Flow files, `ops/flow_run.rs:108,136`; `flow_session.rs:97` | `position.json` owns capture/cursor and review discovery; replacement at `flow_session.rs:301` copies title | Import taskless captures/reviews into the same invocation/Session owner; retain source-independent recovery and exact current attempt |
| Interactive manifest-derived Session | `find_session`, `list_interactive_sessions`, native history and resolved-state checks | General launch reserves Run + Session; SQL lifecycle/title/ancestry reader with Run-owned exact attachment evidence |
| Name sidecar, `run_record.rs:1056,1079` | Read/write `session-name.json`; `carry_session_name` at `human_session.rs:1276` called by Ask and standalone replacements | Session title/provenance survives replacement; convert every writer and import old human names before deleting readers/writers/copy function |
| Derived Work labels, `human_session.rs:1441,1453` | `session_wave_id` and `session_work_path` used by interactive, standalone, Ask and even SQL review DTO projection | Current Run's typed parents, coordinated Rust/Swift DTO fixtures and projection; no selector decoding or cwd regrouping |
| Subject attribution, `run_record.rs:2464` | `preferred_work_selector` still serves Ask inheritance and manifest Work reads | Run-parent inheritance and table queries; retain immutable launch evidence without runtime ancestry fallback |

Attempt history also lacks a production consumer: the public `position_runs`
wrapper currently serves tests. Session/node detail and usage/duration still need
the requested multiple-attempt semantics; storing rows alone does not satisfy it.

## Measured size

Against merge-base `4cd64be3d0432aa04dd9256293eef7088db97ccc`, the review's
reproducible non-test-prefix measure reports **+1,617 / −556** across 18 changed
Rust/Swift production files. This review's delta from `762e546673` is
**+46 / −31**, including the moved shared decoder. **Zero files deleted.**
`ops/human_session.rs` remains **2,350 lines before its trailing test-module
attribute**, or **2,351 counting that attribute** as in Jack's reported boundary.
There was no Session-reader reduction in this review.

Method: `.lf/tmp/loo298-attempts/review-lines.py` excludes test files and terminal
`#[cfg(test)] mod …tests` modules; it retains inline test helpers. It uses text
diffs of those prefixes, not whole-file numstat. These explicitly scoped numbers
do not replace Jack's all-code +4,330/−736 observation or imply a cutover. The
next implementation must report comparable before/after counts and actual deleted
paths; satisfying a line ceiling alone cannot prove removal of authority.

## Next implementation and proof

Complete **Session list and lookup end to end in one pass**. Convert/import Ask,
interactive and taskless Flow Sessions first within that same pass, then switch
list/lookup/rename/actions to the SQL owner and delete the replaced concatenation,
ID dispatch, ancestry reconstruction, title sidecar and title-copy paths with
their obsolete tests. Reuse current Run construction, assignment and Session
transactions. Do not add another owner beside the file readers.

Exercise the actual isolated CLI across all kinds: capture a taskless Flow,
remove its source, recover the review, replace its Run twice while preserving
Session/title/feedback/history, reject stale actions, and consume feedback once.
Include keyed completed Ask retry and interactive rename/bind to a done Task.
Bind must carry the selected Run and exact resolved target through confirmation;
replacement or a competing target must reject without changing either Task's
Started timestamp or artifacts. Import preservation precedes activation of
SQL-only readers; absence of imported data must not silently hide Sessions.

Run affected preservation proofs after schema changes and canonicalization.
Then search reachable launch/read/write/action paths again to demonstrate the
old Session authority is gone. Name any remaining exact dependency instead of
calling a dual-store checkpoint a cutover. Runtime nesting, repository Chapter,
full usage/DTO/desktop work, real-Home maintenance and all eight complete-design
obligations still govern final acceptance. The publication-recording, existing-Task
stacking and cancellation/refused-start reports remain in scope and unresolved.
