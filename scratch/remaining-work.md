# LOO-298: remaining work

Jack Heart · 2026-09-30. Work in this checkout; one code writer. No branch
binary may write the installed Home. Source checks use disposable Homes with
inherited LF_/LOOPFLOW_ authority removed.

## Final model

- Exec owns one actual lf process, its causal parent and command result.
  Each executed Flow step is an ordinary child lf command. Agent-issued
  commands resolve their parent through the current provider generation.
- AgentSession owns one resumable conversation, name, feedback and native
  identity. Captures, provider outcomes, retries and usage are Session events.
- FlowSession owns one started Flow: compiled graph, cursor, return counters,
  selected completion, claim and review. Subflows and loop passes are lenses.
  Task and taskless execution use the same driver. A Task selects one managed
  Flow while permitting other attributed Flows.
- Typed decision output supplies advance, iterate or blocked with a reason.
  Blocked opens a keyed Ask and returns feedback to the same conversation.
- Task attribution is command --as → checkout → ancestor's explicit LF_AS.
  Causal ancestry grants neither attribution nor control. Bind is write-once,
  permits done Tasks and affects subsequent usage only. Work reservation or
  first bind sets Started once; inspection does not.
- A Chapter is the shared name of each Wave's one In Progress Linear Project.
  Project owns its Flow default. Rotation preserves started unfinished Tasks
  and retires only proven untouched backlog. Missing evidence stays unknown.

The core model is implemented; remaining behavior and proof are listed below.
Current contracts belong in
[architecture-reference](../docs/architecture-reference.md) and
[CLI reference](../docs/lf-reference.md); do not reconstruct another design
from the old scratch history.

## Delivery decision

Jack Heart authorized autonomous landing on 2026-09-30: “try to do this all
autonomously, no need to review with me.” Land #1296 as one PR after the checks
below. No demo, concept review or further landing approval is required. This
supersedes the older Task snapshot and demo boundary, not the technical proofs.

#1358, #1359 and #1360 already landed the independent resource, publication and
resident-Wave cuts. The proposed Exec/Chapter splits in `.lf/tmp/landing-groups.md`
would remove only about 10% and require manual extraction; Jack rejected them.
Jack Heart's latest supervising-session steer reserves main integration for the
supervisor after LOO-351's patch release, using `lf sync`; do not merge main in
implementation passes. Never rebase. `94f472815` merged main through #1361. Jack Heart's one-item implement cadence remains in
force: focused proof, checkpoint publication, then the next Flow boundary. The
supervising session owns shipping after this list is complete. Hosted CI now
defers the full matrix while scratch is present; checkpoint publication alone
will not run that proof.

## Before merge

1. **Code and unresolved behavior.** Keep the implemented three-owner model and
   three direct migrations; do not rebuild historical import or split the PR.
   The Task restart stop path now retains the selected child after claim release;
   the real-child proof and its boundaries are recorded below. Client replacement
   now reconnects through the retained live Codex engine; its focused proof is
   recorded below. Workflow review removed the speculative separate engine
   restart: reconnect, client replacement and confirmed-loss retry cover the
   recorded needs. [Incident dispositions](incident-dispositions.md) trace
   Task-row/publication, stacking and refused-start cleanup to current owners
   and focused proofs, including the limits of #1359/#1363. The supervisor owns
   integration before the final gate; the attempted merges described there
   were aborted cleanly. Leave `agent_events` and `exec_events` proposals open;
   neither is a merge prerequisite.
2. **Docs — reconciled locally.** CLI references, builtin skills and `TESTING.md`
   now describe the final model and implemented command tree. Removed speculative
   owner moves, deleted Wave recovery/probe/webhook commands, old Run ownership,
   and nonexistent decision-test instructions. The architecture reference no
   longer lists deleted `run_events` or historical Started imports. Direct
   `--as` contributions may run Flows; checkout attribution precedes inherited
   `LF_AS`. Current-state migration checks and configured-acceptance limits remain
   explicit. Website copies and architecture HTML were regenerated; prompt goldens
   were regenerated from the changed shared surface. Verification is recorded below.
3. **Measurements and behavioral checks — complete locally.**
   [Density measurements and retained proof map](density/README.md) record the
   20,000-Session / 5,000-Flow / 100,000-Exec disposable store, first/warm CLI
   timings, independent SQL and payload controls, raw samples and reproducible
   source. Warm bounded reads measured 291–313 ms against a 300 ms empty-store
   baseline; the Exec search miss measured 362 ms, including a 64 ms SQL control.
   OS caches were uncontrolled and the candidate was a debug build. The estimated
   production delta against recorded main `12013dae4` is +5,287 lines, not a net
   reduction. The map retains one owner per behavior and complementary race tests;
   no tests were added and no further redundant case was established. All 14
   focused Rust and 18 Swift DTO checks passed, with formatting, Clippy and Ruff;
   the integrated gate and configured acceptance remain separate.
4. **Integrated gate — next.** The owned sync has merged main through
   `12013dae4` (v0.12.27); [integration evidence](sync-resolution.md) records
   translated account/steer behavior and focused proof. The waiting sync caller
   owns postcondition verification and push. After remaining repairs, build
   `lf`, run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `uv run python scripts/check_migrations.py` and
   `uv run python scripts/check_architecture.py`. Use the affected-suite plan in
   `TESTING.md` (`uv run python scripts/test.py --list`, then its selected checks):
   full local Rust without fail-fast, release-materialized migrations in a
   disposable source copy, affected Python/website/Swift, app build and CLI smoke.
   Reuse results only for identical content and commands. Required hosted CI and
   merge checks must pass on the final candidate. A missing rendering environment
   leaves configured Desktop proof open; it does not create another review wait.

Recorded compression evidence under `.lf/tmp/test-compress/`: `affected.log`
has 44 passes and two failures; `repairs.log` has both failed cases passing after
repair. `chapters.log` records 17 passes; `native-ownership.log` records two.
Build and final Clippy logs passed. These are earlier focused results, not a
full final-tree gate. Realign inspects this evidence without rerunning the suite.
This documentation reconciliation passed architecture coverage, migration-history
validation (three drafts, 56 shipped migrations unchanged) and `git diff --check`.

Cancellation implementation, 2026-09-30: the existing real-process stop tests
now include an actual child `lf __flow-step` and a disposable scorecard effect,
with both retained and already-released driver claims. The first run reproduced
the missed cancellation (exit 1 instead of 130); the stronger assertion then
exposed release clearing the selected operation. Release now preserves selected
operation/conversation evidence; explicit retry or proven native-exit recovery
resets input. Stop waits for the exact child to exit, retains the cursor and
unknown effect, and recovery requires inspection. Interrupt cleanup fences
mechanical result writes. Task deletion remains the existing LOO-305 operation;
this proof exercises the restart stop core, not a new deletion/stop command.

Focused evidence is under `.lf/tmp/test-compress/`: `cancellation-final-focused.log`
records nine passes covering owned-child exit, handoff, unknown identity,
replacement preservation, retained operation uncertainty and same-pass retry.
The child CLI was built before the library test; source authority was scrubbed
and all data was disposable. Linux's deterministic delayed-cleanup interposition
is authored in the same proof but was not executed by the macOS run.
`cancellation-native-retry.log` records passing driver-and-engine-loss recovery
with the native Codex executable and local synthetic Responses: one conversation
and its earlier history survive explicit retry. The killed request produced a
fixture-server BrokenPipe diagnostic; the protocol/identity assertions passed.
`cancellation-clippy.log`, formatting, architecture coverage, generated HTML
consistency and diff checks passed. Website docs were synced. This is not a final
gate or configured provider/Desktop acceptance.

Client replacement implementation, 2026-09-30: `session connect --replace` had
bypassed the live-engine relay and attempted a separate native resume. It now
stops the exact owned clients and uses the ordinary fenced connection path.
The maintained public-connect proof in `tests/e2e/codex_connect.py` reproduced
that bypass before the fix (`connect-replace-red-3.log`) and passes on the
candidate (`connect-replace-final-tree.log` and its output directory). Evidence
lives under `.lf/tmp/test-compress/`. It retains the active turn, sibling
conversation, native identity, provider generation, history and usage; the next
agent-issued Exec names the replacement driver. Both prior clients exit, and
`--replace --json` leaves clients and ownership unchanged. Earlier red-1/2 runs
were fixture setup failures, not product reproductions. This uses real Codex
0.157.1 with synthetic local Responses and controlled protocol clients, not
configured-provider or rendered Desktop acceptance. The separate engine restart
obligation was subsequently removed by workflow review above. Review also moved
the existing deleted-Task check ahead
of live connection so it cannot be bypassed by a driver handoff.
`connect-replace-retained-task.log` records the retained/deleted-Task regression
passing; `connect-replace-clippy.log` records all-target Clippy passing. Build,
formatting, Ruff, architecture coverage, regenerated HTML consistency and diff
checks passed. Website docs were synced. No integrated gate ran in this slice.

Client replacement compression, 2026-09-30: review resume now shares the same
client-stop operation; unused history and stop forwarding methods are removed.
The public-connect fixture shares client startup without changing its assertions.
Focused verification exposed an Ask fixture opening the default `loopflow.db`
instead of its `registry.db`; the fixture now pins that existing database.
`client-compress-focused-final.log` records 26 passes, and
`client-compress-native.log` plus `client-compress-native/results.json` record the
real-Codex/synthetic-Responses connection proof passing. The initial focused run
also reported one leaky command-construction test; it did not recur in the final
run, and no leak repair is claimed. Evidence remains under `.lf/tmp/test-compress/`.
Build, formatting, Ruff, diff checks and all-target Clippy passed
(`client-compress-build.log`, `client-compress-clippy.log`).
Configured provider/Desktop acceptance remains open. The separate engine restart
obligation was subsequently removed by workflow review above.

`lf pr publish` checkpoint publication is blocked after local code commit `58ea71c2b`.
`lf pr publish` refused: Task LOO-298 records base `00cf9dffe840`, behind the
merged branch fork `4a696c074073`, and requests `lf rebase`. The accepted merge-only
direction remains authoritative. `lf rebase --adopt` only controls an existing
raw rebase; it does not adopt a completed merge. No rebase, direct store edit or
promotion was performed. This is fresh evidence for the remaining publication
incident disposition; #1359 alone did not close this path. The supervising
session needs a supported way to record the merged base before publication.
LOO-351 owns that repair. Jack Heart's supervising session subsequently
authorized plain `git push` (never force) for implementation checkpoints while
it is pending, and retained ownership of the integrated gate and shipping.

Documentation audit, 2026-09-30: source parser review and 88 canonical `--help`
paths passed in a disposable Home with inherited execution authority removed
(`.lf/tmp/docs-reconcile/help-check.py`, `help.log`). Help proves command discovery,
not their external effects. The focused Python architecture/skill checks passed
25 tests; website portable-architecture and README/index checks passed two tests.
The focused Rust builtin and prompt-golden check passed all 15 selected tests
(`.lf/tmp/test-compress/docs-prompt-proof.log`); goldens were generated with
`tests/goldens/update_goldens.py`. Architecture coverage and `git diff --check`
passed. Formatting and all-target Clippy passed (`docs-clippy.log`).
Generated website copies are
ignored output; architecture HTML remained byte-identical after regeneration.
Review caught two additional contract errors: the short guide documented nonexistent
`task list`, and the architecture owner table still named deleted `run_events`.
Both are corrected. This completes item 2 only; dense-store measurements and the
consolidated behavior-proof set were subsequently completed in item 3 above.
Main integration, the integrated gate and shipping stay with the supervisor.
No installed-store write, provider acceptance or rendered Desktop proof is claimed.

## Before a release migrates ~/.lf

Jack reports this machine is the only client. The pinned development Home is
gone; its active Tasks were moved into the one main Home, `~/.lf`, by hand.
Do not recreate that Home or plan a fleet compatibility rollout. This report is
not verification of the moved Tasks or authority for this pass to access the
installed store with branch binaries.

1. **Prepare the exact candidate.** Retain current Waves, Projects, Tasks,
   PR/Linear/worktree links, account routes and resumable conversations. Discard
   finished history, old formats and intermediate drafts. The three remaining
   groups are `record_execs`, `project_status_chapters` and `session_ownership`;
   released migration history stays immutable. Materialize them through the
   release workflow and prove the candidate's actual migration frontier.
2. **Rehearse current-state conversion.** Inspect and adapt the private converter
   `.lf/tmp/deep-compress/convert_current.py` to the final schema and a frozen
   source copy. It currently reads sidecars directly from live `~/.lf` while
   writing a disposable database, so its earlier run is not an atomic snapshot
   or a deployable migration procedure. The SQL migrations alone do not recover
   every resumable native identity from those sidecars. Prove selected captures,
   pending reviews, native IDs, Task links/default Flows, account routes and
   foreign keys on copies, including the manually moved Tasks. Do not copy old
   turns, process receipts or driver authority.
3. **Quiesce and back up.** Coordinate the cutover with the supervising session
   and release operation; autonomous merge authorization is not permission for
   this documentation pass to migrate or promote. Stop new scheduled launches
   and all old writers (Desktop, Task/Flow workers and Session drivers); verify
   owned child exit without inferring authority from ancestry. Back up SQLite
   and required filesystem/native conversation state from the same quiescent
   interval. Retain the matching old executable and a restorable backup.
4. **Apply and verify before reopening writers.** Repeat the proven procedure
   through the authorized release/install path, verify integrity, preserved
   current state and the selected executable/database pair, then resume writers
   on the new version. On failure keep writers stopped and recover the matching
   bytes/store together; never run the old binary against a partly converted DB.

Configured Codex/Claude/OpenCode accounts, interactive reconnect and rendered
Desktop continuity remain release/installed-acceptance obligations. Exercise them
with the candidate and again after conversion where installed state matters;
record failures without claiming fixtures as configured proof. Jack's waiver of
demo/review removes his attendance requirement, not these checks. No live Linear
rotation or distributed transaction is established by local Chapter fixtures.

## Test compression boundary

Keep one maintained proof per final behavior: public Session lifecycle and
binding, headless discovery, Task/Flow retry and review, typed decisions, Exec
ancestry and interruption, and two-Home Chapter convergence. Native Codex tests
retain driver handoff, passive-client fencing, driver-loss recovery, blocked
feedback and structured results against local synthetic Responses. Retired
storage inventories, duplicate reader matrices, removed decision-command trials
and the one-off Chapter rehearsal are deleted. No configured acceptance follows.
