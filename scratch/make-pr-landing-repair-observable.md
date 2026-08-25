# Make PR landing repair observable and terminating

## Finish line

A watched GitHub Actions failure launches `ci-fix` only after Loopflow has:

1. fetched the failing job's hosted log for the exact recorded check URL;
2. proved the Task worktree is clean, on the recorded branch, and at the failed
   head;
3. isolated the provider from ambient Loopflow Run, Home, and writer identity;
4. installed a finite provider deadline; and
5. retained sole responsibility for committing, rebasing, pushing, and
   re-arming a material repair.

The distinguishing proof is a behavioral landing test where a repair provider
terminates without a usable fix: supervision returns, the landing becomes
`blocked` with an actionable reason, and the CI incident carries the same
terminal block. A configured process deadline plus the existing process-group
teardown proof distinguishes termination from a watcher that merely stops
printing. (`provider_completed_at` is hosted-CI timing evidence, not a repair
provider receipt.)

Near-misses that do not count:

- giving the repair agent only a job URL and asking it to fetch logs itself;
- launching in a dirty, wrong-head, or ambient Task environment;
- accepting a scratch-only change, a provider-authored commit, or an unchanged
  tree as a repair;
- timing out the async waiter while leaving a blocking provider thread or child
  process alive;
- logging an error without settling the durable landing and incident.

## Observations

- PR #1237 head `7ab18834a6cfd9ccc25b95df60a477669fd6a759` failed hosted
  `rust-test` job `97668395516` in Actions run `32803308778`.
- The hosted log names exactly two failures:
  `lf::commands::replay::tests::replay_uses_recorded_request_without_the_planning_store`
  and
  `lf::commands::run::tests::ad_hoc_batch_launch_uses_generic_run_record_without_planning_registry`.
  Both failed because `lf` could not be resolved without `LF_BIN`.
- Durable repair run `run_adc545bc7ccb429887c4fdf9e286852b` received only the
  failing check name and job URL. Its `gh` request failed immediately with
  `error connecting to api.github.com`.
- The same run inherited `LF_CONTROL_BIN`, `LF_CONTROL_DB_PATH`,
  `LF_CONTROL_HOME`, `LF_RUN_DIR`, `LF_RUN_ID`, and
  `LF_WORKTREE_WRITER_ID`. It ran `cargo nextest run --all` repeatedly. The
  ambient control and writer state produced unrelated local failures, including
  one run with 150 failures.
- After manually scrubbing the ambient variables and assigning a temporary
  `LF_HOME`, the agent reproduced and passed the relevant test modules. The
  repair trace then ended at `2026-08-25T03:14:01Z` on an unfinished thought:
  no `turn_completed`, error, terminal file, or later conversation event exists.
- `ci_incidents.provider_completed_at` remained null. The landing later blocked
  as an already-consumed repair at `2026-08-25T03:16:13Z`; three `pr land` exec
  spans remained open even though `lf ps --json` showed no owned provider or
  landing process.
- At the incident head, `launch_ci_fix` had no timeout, passed the default
  inherited Run context, did not fetch job logs, and did not prove clean/exact
  worktree state before launch or a bounded material delta afterward.
- The supervisor already publishes via `arm`, so the repair body does not need
  GitHub mutation or inherited Loopflow writer authority.

## Hypothesis

The repair loop delegates evidence collection and isolation to an agent that
cannot reliably reach GitHub and inherits the very local control state that CI
omits. With no provider deadline, a missing terminal event leaves
`run_driver_operation` heartbeating forever. A later supervisor can reclaim the
landing but sees the incident claim as consumed, converting an invisible hung
turn into a generic durable block.

## Design

- Share one GitHub Actions job-reference parser and hosted-log reader between
  `lf wt ci --logs` and watched landing. Fetch `--log-failed` before provider
  launch, retain an exact bounded tail, and block actionably when evidence is
  unavailable.
- Add an isolated agent Run context that removes ambient Loopflow Run, Home,
  control, writer, and Git-operation variables after provider routing is
  configured. Use it only for `ci-fix`.
- Make agent writer acquisition exclusive. A landing repair cannot overlap an
  independently authoritative writer in the same worktree.
- Preflight a clean worktree at the exact failed head and branch. After a
  successful provider turn, require unchanged `HEAD` plus a non-scratch material
  worktree change. Preserve and block on every mismatch.
- Give the provider process a ten-minute deadline. The agent launcher owns
  process-group teardown; only after teardown returns does supervision either
  proceed to Loopflow `arm` or settle the landing and incident as blocked.
- Tighten the builtin `ci-fix` instructions around supplied hosted evidence and
  the smallest named repro. The hosted gate, not a broad contaminated local
  suite, remains the final full-matrix proof.
- Persist one controller-owned receipt with the hosted source URLs and digest,
  provider capture, deadline, and finish time. The incident's existing
  `responded_at` is the sole start time; repaired head, block, green, or merge
  truth derives the displayed repair outcome instead of storing a second state
  machine.

## Restoration proof

- `cargo test -p loopflow ops::pr_landing::tests --lib` proves a repair-provider
  exit returns from supervision and writes the same actionable block to the
  landing and CI incident. The same behavioral fixture proves the controller's
  repair receipt closes as `blocked` after an unusable provider and `repaired`
  only after Loopflow re-arms an advanced head.
- `cargo test -p loopflow ops::pr::tests --lib` proves exact Actions job parsing,
  bounded failure tails, and a terminating hosted-log command.
- `cargo test -p loopflow harness::environment_tests --lib` proves both managed
  harnesses and direct CLI agents lose ambient Run, Home, control, and writer
  authority under the isolated context.
- `cargo test -p loopflow ops::git_operation::tests --lib` proves a second
  authoritative writer cannot launch in the repair worktree.
- On 2026-08-24, the source-built `lf wt ci --logs` run from the PR #1237
  worktree fetched the current `rust-test` failure from its immutable Actions
  job URL through the shared reader. No repair provider was involved.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and
  `git diff --check` pass.

# 5 Whys: A claimed landing repair stranded its watcher

## The Problem

PR #1237's repair consumed the CI incident's one repair claim, lost its provider
without producing a usable fix or terminal result, and left the landing watcher
heartbeating with no actionable account of what was still running.

## Chain

Stranded watcher → unbounded provider wait → unprepared and contaminated repair
turn → generic launch defaults substituted for a repair contract → the durable
model stopped at the delegation boundary → no controller-owned repair-attempt
receipt

**Problem**: The hosted `rust-test` failure remained unrepaired while the
landing stayed live after the provider and local test subprocesses disappeared.
The incident claim prevented a competing repair, so recovery surfaced only later
as "already consumed" rather than as the original failure.

**Why 1**: `launch_ci_fix` supplied no process deadline, and
`run_driver_operation` renewed the landing heartbeat until its blocking closure
returned. The Codex drive waited for a terminal provider event that never
arrived, so child-process disappearance did not settle the repair.

↳ *Could we have caught this earlier?* A failed-repair supervisor test plus a
provider deadline would have made the non-termination observable before the
landing code shipped. The old tests modeled `repair()` only as an immediate
successful function.

**Why 2**: The provider had to discover its own evidence and execution context.
It received a check name and URL rather than the failed hosted steps, could not
reach GitHub, inherited the parent Run/Home/writer environment, and widened its
search to repository-scale tests whose 150 failures mostly described the
contaminated host rather than CI.

↳ *What process allowed this?* The builtin skill explicitly told the provider
to query GitHub and broaden checks. No launch preflight proved the exact head,
clean tree, sole writer, or reproducible environment first.

**Why 3**: Landing repair reused the generic `AgentConfig` and `ProcessConfig`
defaults: inherited authority and no timeout. Its output contract was also
implicit—any zero exit advanced to `arm`, with no check that the provider left
an uncommitted material delta on the failed head.

↳ *What assumption was wrong?* The code assumed a repair agent was a trusted
continuation of the landing process. It is a fallible child with different
evidence, authority, lifetime, and acceptable output.

**Why 4**: Durability covered the GitHub observation, incident claim, landing
generation, and eventual repaired head, but not the interval between claim and
re-arm. The synchronous call stack was the only link between the CI incident
and the provider capture. If that stack vanished, durable state could say
`repairing` without naming the attempt, its deadline, its evidence source, or
its terminal result.

↳ *Why was that assumption encoded?* Heartbeats and one-shot incident claims
were designed to prevent duplicate supervisors. They fenced ownership, but
ownership was mistaken for worker progress and completion evidence.

**Why 5 (Root)**: Loopflow had no controller-owned repair-attempt boundary. A
delegated control action was modeled as an opaque agent call instead of one
durable transition with immutable inputs, least authority, a deadline, an
accepted output, and a terminal receipt. The system could therefore preserve
the right to repair while losing the facts needed to finish or explain it.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 1 | Why did the Codex process disappear without emitting a terminal turn event? This is provider/Run settlement work owned by LOO-265; landing must tolerate it rather than depend on its answer. | High, separate |
| Why 2 | Why did the triggering PR's hosted tests require `LF_BIN` while the developer repro did not? This explains the CI regression, not the landing strand. | Low |
| Why 4 | Why did three `pr land` Exec spans remain `running` after their OS processes disappeared? Global Exec reconciliation is broader than this landing repair. | Medium, separate |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Fetch the exact hosted failure before launch; require the recorded clean head; bound evidence and provider execution; durably block every unusable exit. | Another PR #1237-style invisible repair stall |
| Structural | Launch `ci-fix` with isolated authority and one writer, require one material uncommitted delta, and leave commit/push/re-arm to the landing supervisor. | Contaminated repros and provider-owned publication |
| Systemic | Persist one repair-attempt receipt linking incident, evidence identity, provider capture, deadline, and terminal outcome; make `lf ci` render it. | A controller retaining ownership while losing the delegated action's observable lifecycle |

## Changes to Implement

- [x] Fetch bounded hosted failure logs from the confirmed check URLs before
  provider launch.
- [x] Prove clean branch/head state, exclusive writer ownership, and isolated
  Run/Home/control context.
- [x] Install a provider deadline, validate the material result, and settle
  landing plus incident as blocked on every unusable exit.
- [x] Prove a failed provider cannot keep supervision waiting and that process
  teardown reaches provider grandchildren.
- [x] Add a durable repair-attempt receipt and expose its evidence source,
  capture, deadline, and terminal outcome through `lf ci`; do not change the
  generic provider terminal-event semantics owned by LOO-265.

## Review slice

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Exact hosted evidence | Fetch each failed Actions job before provider launch and retain a bounded tail. | The shared Actions reference reader runs `gh run view --log-failed`, selects the recorded job, applies a 30-second command deadline, and hashes the bounded evidence into the repair receipt. | Source-built `lf wt ci --logs` read the live PR #1237 check boundary; the 2026-08-24 restoration run fetched its `rust-test` failure from the immutable job URL. `cargo test -p loopflow ops::pr::tests --lib` passes 28 tests. | pass |
| Clean, isolated writer | Launch only at the failed branch/head with a clean tree, no competing writer, and no ambient Run/Home/control authority. | Landing preflight checks branch, head, and cleanliness; agent writer acquisition is exclusive; `AgentRunContext::Isolated` scrubs both managed-harness and direct-agent environments. | `cargo test -p loopflow harness::environment_tests --lib` passes 4 tests; `cargo test -p loopflow ops::git_operation::tests --lib` passes 2 tests. | pass |
| Finite provider lifetime | A missing terminal provider event cannot hold the watcher forever or leak the provider's descendants. | `ci-fix` has a ten-minute process deadline. On timeout the Codex drive is dropped, then `harness.stop()` kills and waits for the provider process group before returning. | `cargo test -p loopflow harness::codex::tests::kill_process_group_reaches_the_grandchild --lib` passes. `cargo test -p loopflow ops::pr_landing::tests --lib` proves the watcher returns and blocks after an unusable provider. | pass |
| Bounded repair output | The provider leaves one material uncommitted change; Loopflow alone commits, pushes, and re-arms it. | The result must remain on the failed head and contain a non-`scratch/` delta. Any commit, empty result, or re-arm that does not advance the head blocks durably. | Landing behavioral tests cover failed-provider blocking and successful re-arm to an advanced head; the builtin `ci-fix` contract forbids commit, push, land, or merge. | pass |
| Observable terminal receipt | Link hosted evidence, provider capture, deadline, and finish to the incident without duplicating its lifecycle. | The incident claim atomically stores the repair facts. `finished_at` closes only with repaired, blocked, green, or merged truth; `lf ci` derives and renders `running`, `repaired`, `blocked`, or `superseded`. | `cargo test -p loopflow lf::commands::ci::tests --lib` and `cargo test -p loopflow ops::pr_landing::tests --lib` pass; the latter materializes and executes the draft migration. | pass |

The configured live demonstration was intentionally read-only: the source-built
`lf wt ci --logs` resolved the current hosted checks and exact Actions job URLs,
which are now green. Mutating repair settlement is therefore proved through the
production store/supervisor path in the landing tests rather than by manufacturing
a new hosted failure.
