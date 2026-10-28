# Rust tests

Focused checks and fixture rules for `rust/loopflow`. Ordinary suites join
`tests/integration.rs`; standalone binaries are registered in `Cargo.toml`.

## Toolchain

Match CI's current stable toolchain before accepting a local lint pass:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
rustup update stable --no-self-update
rustc +stable --version
cargo +stable clippy --version
cargo +stable fmt --all -- --check
cargo +stable clippy --all-targets -- -D warnings
```

The installation harness pulls `docker.io/library/rust:trixie`
from the official Rust Docker Hub repository. CI fingerprints the acquired local
image ID before restoring its Cargo cache; acquisition failure is infrastructure
failure, not an installation-test result.
It follows stable alongside CI's Rust jobs and logs its compiler version. Trixie also supplies
Git with `merge-tree --merge-base`; Bookworm's Git 2.39 cannot run stacked replay
checks. After adopting a newer
standard-library API, run `uv run python scripts/test_task_installation.py` too;
a local lint pass does not prove that the disposable installation builds.

CI installs stable on each run. An older local compiler can miss new Clippy
lints and standard-library deprecations. Put rustup's proxies first on `PATH`
so Cargo subcommands cannot select an older Homebrew Clippy or rustfmt.

## Boundary checks

For migration changes, run `uv run python scripts/check_migrations.py` before
the Rust suites. Schema construction can pass even when a released migration
was edited; this check also verifies immutable history against main and tags.

`cargo test -p loopflow --test integration agent_startup_tests::` exercises bare `lf` and
Session reconnect to a stand-in provider with an empty environment and no
credentials. CI's ordinary Rust network isolation covers it. It bounds provider
and Git launches, SQLite statement/returned-row counts, and time to provider
handoff; [native readiness measurements](../../../scripts/benchmarks/agent-startup/README.md)
remain a separate opt-in benchmark.
Run this check when changing transport ownership or process journaling: every
added CLI process pays connection and schema setup costs, including at exit.
Journal connection changes also run `cargo test -p loopflow --lib journal::tests`
to preserve completion and interruption receipts after database contention.

Changes to Git default-branch discovery, checkout roots, or commit/push behavior
also run `agent_startup_tests`, `release_tests`, and `release_lock_tests`. These
cover startup cost and release retries that share the same Git operations.

Changes to builtin Graphs affect the parser, graph, Task controller and CLI fixtures.
Run the affected controller progression and CLI behavior tests as well as graph
checks; use authored fixture Graphs when a test needs a fixed sequence independent
of product defaults.

Changes to interactive/headless selection also run `default_conversation_tests`
and `context_launch_tests` alongside `graph_tests`: bare `lf`, explicit `-b`, and a
Graph invoking the `default` skill must preserve their distinct launch modes.
Prompt fixtures read the harness's actual inputs, including context files and
stdin, rather than assuming everything remains in argv.

Context byte-budget fixtures must reach their boundary with short temporary
paths too. Run `TMPDIR=/tmp cargo test -p loopflow --lib context_block` when
changing them; macOS's longer default paths can hide an undersized fixture.

The composed delivery fixture runs a real CLI/Graph against an isolated Home and
simulated provider observations, ordinary Task/comment saves and independent completion,
then checks its captured CLI and monitor population in Rust
and Swift. It builds the CLI with Cargo so a unit-test binary cannot accidentally
launch an installed version. Regenerate its wire capture with
`LOOPFLOW_UPDATE_LIFECYCLE_FIXTURE=1 cargo test -p loopflow --lib ordinary_task_comments_and_completion_preserve_live_graph_and_arrival`,
then run `cargo test -p loopflow --test integration dto_fixtures::composed_lifecycle` and
`swift test --package-path swift --filter 'composedTaskLifecycle|dueDate'`.

The Work-first entry capture comes from isolated repository-primary admission,
reopening and mixed Session history. Regenerate it with
`LOOPFLOW_UPDATE_WORK_FIRST_FIXTURE=1 TMPDIR=/private/tmp uv run python scripts/test_network.py cargo test -p loopflow --test session_lifecycle_tests work_first_repository_entry < /dev/null`.
`scripts/test_desktop.sh --filter 'WorkNavigationTests/composedWorkFirstEntry' -Xswiftc -gnone`
consumes that capture through the repository-entry callback and retained Work owners.
This checks configuration and identity, not a mounted provider or native usability.

Provider fixtures must read the launch's actual context channel. When a final
sync adds fixtures that inspect a changed transport, run those focused tests
before arming; the earlier gate did not cover the newly combined behavior.
Context-hook changes also run `session_cli_tests`: resume callbacks supply the
native Session identity and event payload, just like startup. Provider fixture
scripts must fail when a callback fails, rather than report a successful turn
with missing context evidence.
OpenCode context-hook changes also run
`node scripts/benchmarks/skill-invocation/test_opencode_context.mjs` for refreshed
system context after compaction, preserved user messages and child-session isolation.
Pi context-hook changes also run
`node scripts/benchmarks/skill-invocation/test_pi_context.mjs`. The isolated real-Pi
probe in `scripts/benchmarks/pi/probe.py` takes `--pi /path/to/pi` and covers failed/empty
results, automatic overflow compaction, native continuation, Unicode PTY input and
public live steering. It uses a local model and disposable homes; configured-account
and rendered acceptance remain separate.
Claude stream fixtures must acknowledge the input's UUID before completing its
result, and stay alive until the harness closes stdin. Include
`explicit_launch_policy_survives_task_workflow_nested_graphs_and_provider_children` when
integrating Claude transport changes; bound the test so missing acknowledgements
fail locally instead of exhausting the merge queue's deadline.
Cross-provider replay fixtures must install stand-ins for both the recorded and
override providers, with a fixture/system-only PATH. A developer's installed
provider must neither satisfy a missing fixture nor receive a test launch.

## Linear acquisition fixtures

Linear acquisition fixtures live under `rust/loopflow/tests/linear/` and compile
inside the library, where fetched response types stay private. Focused Cargo tests
build the current CLI and take its path from Cargo’s artifact output:

```sh
cargo test -p loopflow --lib local_planning
cargo test -p loopflow --lib linear::acceptance
```

CI and the disposable installation runner supply their prepared candidate through
`LOOPFLOW_TEST_LF`, avoiding Cargo calls in each test process. Only set this to
the executable built from the current test source and build inputs; never an
installed CLI. Compiled provenance fields have separate names from build inputs
so nested Cargo does not invalidate the parent's build.

These fixtures still invoke the built CLI with disposable Homes. Shared execution
fixtures seed fixed admission evidence; they do not expose acquisition APIs.
Queued multi-Wave responses must pin distinct Wave creation times to match their
newest-first discovery order; same-second creation can hide an ordering mismatch.
Paused-clock tests that own real subprocesses must poll startup and advance time
explicitly; child I/O can prevent Tokio's clock from auto-advancing.

For shared repository discovery or CLI dispatch changes, include the PM and
Wave consumers in the focused check:

```bash
cargo test -p loopflow --lib wave_resolution
cargo test -p loopflow --test global_commands
```

When adding a command with `--wave`, classify its selection in
`wave_resolution_matrix` and run that suite alongside the command's own tests.
Cover repository defaults and explicit Wave selection separately from ambient
Wave resolution.

Preserve fixtures for registered Wave directories without Git metadata. Adding
Git would hide the cached-PM context regression; global-command tests alone do
not cover it.

Work-command dispatch also needs the registration lifecycle and repository
ownership tests. Empty registrations can be forgotten from their registered
directory without Git metadata; Work operations still enforce repository ownership.

```bash
cargo nextest run -p loopflow --lib -E 'test(status_tests::) | test(wave_repository_ownership::)' --no-fail-fast
```

Task decision feedback has a focused store-and-Graph-runner check:

```bash
cargo test -p loopflow --lib task_decision_live_unblock_returns_feedback_without_navigation -- --test-threads=1
```

It completes a keyed Ask while the decision is active and verifies that feedback
returns to the same captured conversation without choosing Graph navigation.
Provider effects are simulated. The name's `live` refers to the active local
decision, not configured-provider acceptance. Structured decision output owns
Advance, Iterate, or Blocked with a required reason; feedback alone is no verdict.

Task stall check uses `session_record::activity::tests`, `task_live_unblock`, and
Swift `TaskGraphProofTests`. The sampler retains PID/start identity and cumulative
CPU for the body and descendants in Session events. Five quiet minutes
requires samples no more than 45 seconds apart; the worker samples every 15
seconds. Unit tests advance a simulated clock; the CLI/desktop check samples a
real sleeping process with a seeded five-minute history. Neither establishes a
five-minute configured provider stall. Preserve CPU-active silence, fresh-event,
missing-sample and PID-reuse counterexamples when changing the projection.
Repeated samples of a reused body PID must remain Unknown; they cannot replace
the observer's original body identity and later establish a stall for that captured input.

See [boundary-specific checks](#boundary-specific-checks) for Task controls,
Linear response shapes and isolated Session fixtures.

`process_ownership_tests::interruption_records_the_process_without_a_fabricated_signal_name`
checks the OS exit, durable Process outcome and absence of its owned scorecard child.
It retains child output for failures. On Linux, `cc` builds the test-only
`support/hold_group_kill.c` interposer: after delivering the real group kill it
holds the signal hook for 200 ms, exposing normal command return racing cleanup.
The fixture asserts that this scheduling point was reached. This is controlled
ordering evidence, not a claim about how long a hosted signal handler paused.
Other Unix platforms exercise the ordinary interruption path.

Exercise a deciding step's structured output with real Codex and a local
Responses fixture:

```bash
uv run --script tests/e2e/codex_connect.py --codex "$(command -v codex)" \
  --lf target/debug/lf --launch --graph-decision-retry replace \
  --output .lf/tmp/native-graph-decision
```

`--graph-decision-retry missing` exhausts the corrections of an invalid answer;
`replace` proves a failed turn decides nothing. `--public-connect` covers a
live headless-to-terminal handoff.
`--shared-provider-home` proves Loopflow and plain Codex share one home signed
in as one stored account at a time: switching, saved-back logins, isolation,
and provider conversation IDs in `lf session`.
`tests/e2e/claude_shared_home.py --claude "$(command -v claude)" --lf
target/debug/lf --output <dir>` proves the same for Claude with synthetic
logins and a local endpoint: a Claude started after a switch sends the login
Loopflow installed, a shared launch gets no account home or credential
variable, and an isolated launch stays in its account's home. On macOS it
writes and removes one Keychain item scoped to its temporary config directory.
The fixture copies the candidate, uses private Machines and stops only its identified
provider process children. Native execution uses synthetic Responses, not configured
accounts or installed data. Ordinary retry, usage, binding and review behavior
belong in `session_lifecycle_tests`; Chapter convergence belongs in
`ops::chapter::tests`, including interrupted rotation and second-Machine sync.

`planning_reconnect_tests` runs public work-watch and Graph-boundary recovery against
synthetic Linear on Linux, with disposable TLS trust. It covers lost replies,
independent acquisition during failed delivery, local buffering and retained execution.
The peer implements team deltas with current revision timestamps. Graph acquisition
waits for a turn/step boundary; only watchers retry on the 30–240-second clock.
Run these Linux fixtures when changing Linear acquisition, even if macOS checks pass.
Linear and Git planning run separately; no provider receipt crosses Git.
`planning_setup_tests` proves configuration selection and personal-key recovery.
`planning_foreground_tests` exercises disposable custom refs through public CLI and
Session connections, including offline independent edits, comments, completion and
unchanged execution. `planning_write` covers atomic local journal capture and import
through the same typed writer. `task_remote_tests` exercises actual target CLI dispatch
with simulated SSH/providers; it does not establish configured remote installation.

`tests/e2e/codex_connect.py --codex "$(command -v codex)" --lf target/debug/lf
--planning-peers --output <dir>` composes a real Codex engine with a synthetic
Responses endpoint and disposable Git planning. It launches with `--task`, then
imports edits, comments and completion for that same Task during live connect and
explicit resume, including after killing the fixture's driver. Its historical
Workflow is seeded; its Session, drivers and checkout are created by public commands.
Exchange preserves Workflow/placement, live driver, Session/native identity and
append-only history without starting an engine or provider turn. After imported
completion, the current client explicitly starts and completes another turn on the
same engine. The controlled protocol client is headless, not a rendered TUI or
running Workflow-edge proof. Prepare the script's uv dependencies before
running it through `scripts/test_network.py`.

Only attempted effects acknowledge; equal-desired provider readback can instead
retire an unattempted intention without another write. The portable `watch` offline-completion test requires a foreground delivery error
with no Task selected; local frame propagation alone cannot establish sync lifetime.
Run these through the same external-network denial wrapper as other CLI fixtures.

Task-planning fixtures also need an explicit `LF_BIN`: Task status validates
launch authority before reconciling a user merge. Pin the test executable when
no child is launched; an installed `lf` on PATH can hide this missing fixture.
Hook-declaration fixtures likewise need a disposable `LF_HOME` and explicit
`LF_BIN`, even when they never execute the declared hook. When changing executable
selection, run these fixtures with installed `lf` directories removed from PATH.

After Project ownership or uniqueness changes, run the Task-planning consumers.
Planning sync already creates the local Project;
registered-Task fixtures must reuse it rather than insert a second owner.

Planning-owner changes must cover context/catalog reads, Wave relocation, Task
initialization, Workflow completion/reopening and PR delivery fixtures alongside
the new planning APIs. Write Wave goal/memory and `.lf/graphs/` fixtures directly in their checkout;
writing a Workflow file is not a planning mutation.
An existing checkout fixture must settle initialization before testing delivery.
Prompt fixtures nested in the source tree must fence Git discovery so the enclosing
checkout cannot become their stored Wave owner. Verify them inside a Git checkout;
a source archive alone does not exercise that boundary.
Include the shared DTO fixtures and Desktop planning states in the affected gate.
Repository-wide Linear fixtures must return comments only for their owning issue;
exercise acquisition in a different order from Task creation. Process-sensitive
cleanup fixtures must observe child readiness before inspecting live executables.

```bash
cargo nextest run -p loopflow --lib -E 'test(ops::linear::task_planning_tests::)' --no-fail-fast
```

Every CLI fixture must select an explicit disposable `LF_HOME` and pin
`LF_BIN` to the compiled test CLI. Clear inherited `LF_*` execution authority.
Without an explicit private Home a source CLI works on the main Machine's store.
Children use the same Machine and executable; PATH cannot choose a
second store. A Machine's database is always `$LF_HOME/loopflow.db`.

When editing the repeated Task body, exercise every step on two passes and
saved-decision recovery. Keep loop-or-next after work and review so navigation
cannot skip a later review.

Include the CLI/desktop unblock projection when changing builtin Graph composition:

```bash
cargo test -p loopflow --test integration task_initialization_tests::task_live_unblock
```

Fixtures targeting a named boundary should resolve its node ID in the expanded
graph. A hard-coded step index can silently select a different skill when
a nested Graph gains a step.

For worktree creation or checkout-refresh changes, build the current CLI before
running its Python behavior tests:

```bash
cargo build -p loopflow --bin lf
LOOPFLOW_TEST_LF="$PWD/target/debug/lf" uv run pytest python/tests/test_checkout_refresh.py
```

Checkout pruning also reads OS-account installation authority. `LF_HOME` alone
does not isolate that read: an unreleased CLI can reject the installed Home's
older schema. Run these proofs in a disposable OS account or leave them to the
Rust CI job; never migrate the installed Home to make a fixture pass.

The background-push regression holds Git until the CLI exits, then verifies
upstream tracking and a subsequent sync. Immediate local pushes can hide
broken pipes that interrupt Git after the remote ref moves; background children
must use stdio that survives their parent's exit.

Builtin skill Markdown is compiled into Rust. After editing it, run the builtin
contract tests alongside the relevant behavior tests; PR renderer tests alone do
not cover the prompts that generate their input. Update obsolete assertions to
match the intended contract instead of restoring retired commands in the prose.

```bash
cargo test -p loopflow --lib builtins::tests
```

When changing the builtin catalog or Graph composition, also exercise discovery.
Listings keep effective skills and authored graphs separate and show their sources
and invocations; `lf graph show` expands a graph's steps.

```bash
cargo test -p loopflow --test integration cli_discovery::list_preserves_kinds_overrides_sources_and_reserved_invocations
```

Catalog retirement also affects historical migration tests. Keep their persisted
names and data-preservation assertions at the migration boundary; current catalog
resolution belongs in skill catalog tests. Include the legacy Graph repair check:

```bash
cargo test -p loopflow --lib legacy_task_graph_repair
```

Skill export tests use isolated homes and cover builtin/global definitions,
personal agent directories and pruning. `sync-skills --repo` exports repository
skills and Graph recipes into the checkout; default sync exports globally.

```bash
cargo test -p loopflow --lib skills::tests
```

After editing embedded skills, directions, surfaces, or prompt assembly, run
the Rust golden prompt check even for Markdown-only changes. If the mismatch
reflects the intended prompt change, regenerate the snapshots, review their
diff, and rerun the check before gate.
Terminology-only replacements count as prompt changes; include the golden
check in their focused verification even when no prompt assembly code changed.
Skill/Graph naming changes also update architecture command examples: check both
the prompt goldens and portable architecture HTML before arming, even when the
focused discovery and export suites pass.

```bash
uv run --project website python scripts/render_architecture_html.py --check
```

```bash
cargo test -p loopflow --test golden_prompt
uv run python tests/goldens/update_goldens.py   # refresh prompt goldens after prompt changes
```

For document gathering or deduplication changes, run the complete context suite
alongside the delivery checks; a test-name filter alone misses ancestor coverage.
Rendering gathered documents must include the production deduplication step.
Keep the library `context_delivery` filter in the gate plan: it covers direct
and Wave launches in `ops::run`, outside the prompt modules. Check ambient and
explicit selections across both the refreshed block and submitted turn.

```bash
cargo test -p loopflow --test integration context_tests::
cargo test -p loopflow --lib context_delivery
cargo test -p loopflow --lib skill_launch
```

Run the launch checks after syncing CLI changes; they parse the current flags
before verifying delivery to the provider.

Changes to builtin `LOOPFLOW.md` affect every prompt golden. Regenerate and
review them before gate. The `skill_launch` checks above cover terminal launches.
Keep prose contracts in builtin tests; launch tests should prove that the
canonical document is included.
For migration regressions, inspect historical fields at their migration boundary
(`apply_before_migration`), then finish the upgrade and verify the current schema.
When chapter triggers change, include Task consumers: durable work reservation
retains its ownership claim after failure. A private Home that applied an edited
unmerged migration fails its checksum: use a fresh one. Preserve populated
historical fixtures for merged migration coverage. A Task's migration is edited
in place, so test the merged frontier against the finished migration and write
no test for a schema that existed only between two edits.

A source copy without Git metadata must set `LOOPFLOW_BUILD_PROVENANCE=development`
when exercising source behavior, and `LOOPFLOW_MIGRATION_ORDER` when it carries
slug migrations; it also cannot prove fixtures that require `git rev-parse HEAD`.
Leave the live Machine untouched.

After adding, renaming or removing a public command owner or concept, run
`uv run python scripts/check_architecture.py`;
retained tables and subprocesses still need their actual owners in the map.
For architecture/README documentation changes, run
`cd website && uv run python dev.py test -k 'portable_architecture or readme_index_sync'`.
Model renames also require this focused website check; generating matching HTML
does not catch stale vocabulary in the website assertions.
For Markdown changes under `docs/`, builtins or skills, and changes to README or
AGENTS, also run `cargo test -p loopflow --test integration documented_commands::`. Its command
scanner covers prose and headings as well as fenced examples; a focused runtime
suite does not check those additions.
`docs/lf-reference.md` commits only its preamble; the website's `sync-docs`
appends the command tree from `lf help --all --json`, so a command or help-text
change needs no reference update.
Keep README and docs/index openings identical and regenerate docs/architecture.html
when its source changes. Retired Project surfaces also affect CLI fallback,
builtin discovery, prompt goldens and storage settlement.

For landing changes, prove same-head recovery and authoritative merge separately
from commit creation. Exercise takeover while the old repair is still running;
generation fencing alone does not stop its effects. Label simulated
provider/GitHub tests. Known live-check limits belong in Infrastructure memory.

Landing changes also affect release finalization and Linear completion recovery:

```bash
cargo nextest run -p loopflow -E 'binary(release_tests) | test(ops::linear::task_planning_tests::task_completion) | test(ops::pr_landing::)' --no-fail-fast
```

Detached repair proofs enter through the compiled CLI so admission records its
Process before launching a child. Assert the published tag and completed repair;
an in-process `release_run` call does not exercise that execution boundary.
Include stale failure observations after repair completion: a fresh authoritative
merge must still settle the release and publish its tag, even if repair admission
has already blocked the completed incident. Use fixture synchronization to prove
the ordering without timing-dependent sleeps.

Run CLI-backed Python tests only after the Rust build finishes; replacing their
binary mid-test mixes migration frontiers in a single temporary Machine.

Fresh-store coverage exercises the live SQLite schema. Populated historical
fixtures exercise the migration chain and verify retained facts. A fresh-store
pass alone does not prove that an existing Machine can upgrade without losing work.

Prove explicit experimental Machine continuity with `global_commands` and
`one_machine_tests`. Use a fresh `LF_HOME` for each schema version: source binaries
without an explicit experiment forward to the installed CLI and main Machine.
Never run candidate mutation checks against the main Machine.

Run the Task CLI proofs with isolated installation authority:

```bash
uv run python scripts/test_task_installation.py
# One changed Task adoption proof:
uv run python scripts/test_task_installation.py --test task_adopts_linear_checkout_and_preserves_graph_history
```

Pass several names after `--test` to share one disposable build across related proofs.
Agent-selection and Task launch-mode changes also require the container-only
`declared_agent_can_start_another_tasks_graph` proof: saved Task defaults remain
local, while explicit `-a` overrides child Tasks too. Ordinary Rust suites skip
this installation-boundary proof.
Fixtures that stop at an interactive step use bare `task run` and wait for the
recorded Graph outcome; `-b` executes that step headlessly and needs a provider.
Every ordinary installation run first upgrades populated released planning and
Task/PR fixtures through every later migration. Those checks retain zero/one/multiple
historical PRs, pending completion delivery, a running Process and its Workflow,
and an associated Session; they never open an installed Home.
The default container tracks stable Rust, matching the other CI Rust jobs, and
logs its compiler version. When adopting a newer standard-library API or Clippy
fix, verify the installation harness uses the same toolchain policy; a host lint
pass does not verify the container build. Use `--image` to reproduce an older
toolchain explicitly.

Locally available images are reused, including between the two CI proof steps;
run `docker pull docker.io/library/rust:trixie` to refresh the default
on a development machine. Cold pulls retry registry throttling with bounded
backoff within a five-minute download budget. Exhausted acquisition reports an
infrastructure failure before any installation tests run.

CLI owner-tree changes must include `cargo test -p loopflow --lib graph::compiled::tests`
to verify builtin operation labels, plus the affected proofs above. The regular
Rust suite skips those installation proofs; a skipped case is not verification.
Task status reads a Graph from recorded command outcomes and OS liveness. Run
`cargo test -p loopflow --lib ops::task_execution::tests` for running, between
steps, stopped and failed.
Managed Task fixtures must bind the checkout's Team and Initiative before
creating Task worktrees; reuse `support::bind_task_planning` for the shared fixture.

When changing Task planning lookup or provider response shapes, run the affected
installation proofs and the paired `local_planning` deletion proof. The portable `task_abandonment`
fixture retains unresolved Sessions, Processes, published PRs and files in both
connection modes. The regular Rust suite skips installation proofs. Keep
simulated provider revisions and checkout Team/Initiative bindings consistent
with the planning records those workflows resolve. Exercise unfinished work
before confirmed removal; do not resurrect deleted Tasks by resetting only
execution tables while retaining planning tombstones.

This copies source into a disposable Linux container with its migration order
and builds the development CLI with two Cargo jobs. It checks
populated planning upgrades from the released schema. Intermediate branch
schemas follow the current-state cutover policy and are not imported. Installation-copy succession is no longer a
product contract; its two cross-store promotion/continuation cases were removed.

The adoption case starts with planning and no Task row. Public checkout/run
reuse a dirty existing Git worktree or fetch an open PR's unseen remote branch,
retain the PR identity, and leave an earlier Graph's Processes untouched after source
changes. Linear planning and GitHub reads are fixtures; Git and CLI paths are real.

The disposable OS account authors fixture installation records for routing proofs;
Task adoption uses an explicit experimental Machine. No host Machine, credentials or installation
is mounted. Default executable routing uses two real source CLI processes and
a simulated installed executable. It runs in this disposable account because
`HOME` and `LF_HOME` cannot isolate installation records. The ordinary
`pr_tests` suite covers changed-head auto-merge revocation and review
continuity in an explicit experimental Machine.

The separate planning CLI proof (`cargo test -p loopflow --lib planning_lookup_tests`) runs planning-only `task status` by identifier and UUID against
normalized local planning, proving that inspection creates no execution or worktree.
That CLI case exercises cached planning; `ops::linear::planning_lookup_tests` covers
acquisition with simulated Linear responses, including missing Projects, partial
responses, absence, and provider failure. Neither is configured live-provider proof.

The mechanical Graph proof uses that disposable account without an installation
selection. A work reservation that fails before allocation leaves the Task unowned;
the real worker records operation history and its ownership claim together. It retains the
captured Graph after the template disappears and records an ordinary child Process without creating an LfSession.
The default-Machine proof sends two nested source CLI processes through a simulated
installed executable and checks that both select the main Machine despite stale
control pins. The explicit-Machine Graph proof runs locally in `one_machine_tests`. The declaration proof starts Task Y
from Task X's agent and checks Y attribution while retaining X as the causal parent.

The harness keeps Docker build/registry caches, serializes use of its build
cache, and destroys the account and installation after each attempt.

Session history has focused storage, harness, reducer, and reader checks:

```bash
cargo test -p loopflow session_record
cargo test -p loopflow journal
cargo test -p loopflow store
cargo test -p loopflow harness::conformance_tests
```

When changing harness event mapping, run the recorded-trace conformance tests
alongside the provider's unit tests. Keep trace expectations aligned with the
event contract, including durable final-answer receipts and usage checkpoints.

After Session-history or schema changes, run `lf history list --json`, `lf usage --json`, and
`lf doctor --json` against a disposable Machine with inherited `LF_*` and
`LOOPFLOW_*` authority removed and `LF_BIN` pinned to the compiled source CLI.
Never use the installed store to prove an unmerged migration.

## Codex daemon account switching

```bash
cargo test -p loopflow --lib lf::commands::account::daemon_tests
cargo test -p loopflow --lib expired_grace_restarts_even_when_activity_becomes_unreadable -- --ignored
```

Run the second check explicitly at gate when changing daemon login reconciliation.
It takes the real five-minute grace period, bounded by a 315-second test timeout.
Both checks use disposable Unix sockets and a stand-in restart executable, without
credentials or a live provider. They cover busy-to-idle, unreadable probes, expiry,
and adoption after restart; configured Codex behavior remains demo/review evidence.

## Boundary-specific checks

Current-state conversion preserves resumable conversations and current planning;
it does not reconstruct finished history from retired owners. Keep these populated
migration proofs when changing the three-owner schema:

```bash
cargo test -p loopflow --lib ownership_cutover_keeps_current_task_review_without_importing_history
cargo test -p loopflow --lib project_status_adoption_preserves_current_identity_and_custom_graph
```

They check retained review identity/capture and the current Project's custom Graph
default. Synthetic migration success does not authorize conversion of an installed
Machine or prove configured-provider resumption.

When changing how a Graph step is described or read back, include the step
argument and Process inventory tests and the public Session lifecycle proofs. A
Graph is its Graph process and step Processes; assert on those Processes and on the Session
turn a step Process captured.

```bash
cargo test -p loopflow --lib ops::graph_run
cargo test -p loopflow --lib store::sqlite::graph_inventory
cargo nextest run -p loopflow --test session_lifecycle_tests --no-fail-fast
```

When changing Task controls, include the GitHub-cache integration tests as well
as controller tests. Bare interrupts prove local control during GitHub outages;
steering publishes to Linear and belongs with the mocked Linear boundary tests.

```bash
cargo nextest run -p loopflow --test integration task_github_cache_tests:: --no-fail-fast
```

When changing Linear response shapes, run the client tests and PM-operation
consumers together, including `planning_reconnect_tests` on Linux (a disposable
container works on macOS). That suite is explicitly ignored on macOS, so a local Rust
pass there does not cover its HTTPS fixtures. Keep nullable requested fields,
including `dueDate`, present in fixture responses. Team migration also reads issue
comments; its fixtures must include the requested pagination metadata.
Run the same Linux suite when changing Linear mutation inputs or acknowledgements:
the reconnect peer must accept partial Initiative updates and return `success`.

```bash
cargo nextest run -p loopflow --lib -E 'test(linear::) | test(ops::linear::) | test(ops::linear_observe::)' --no-fail-fast
cargo nextest run -p loopflow --test planning_reconnect_tests --no-fail-fast
```

### Test without an installed Loopflow

Session-command fixtures must work without an installed `lf`. Supply an `LF_BIN`
fixture, restore it afterward, and serialize environment changes with
`test_env_lock`. Reuse `TestLfBinGuard` in Task controller tests and `SessionHome`
in conversation tests; keep Session spawning mocked. Listing waiting Sessions
also resolves the executable for their open command, even with spawning mocked.
When a provider launch fixture only resolves `lf` without invoking it, pin
`LF_BIN` to `std::env::current_exe()`. Verify it with the test executable's PATH
restricted to system tools (`/usr/bin:/bin`), so an installed `lf` cannot hide
missing fixture setup; keep Cargo's build PATH unchanged.
Enter `journal::with_runtime` after selecting the fixture Machine so its Process and
the Session attachment references share the same database. Simulated finite-provider
harnesses must record their owned child exit; an absent endpoint is not exit
evidence.

Launch proofs use provider-written evidence and terminal receipts.

### Reproducing races without CPU load

Never generate CPU load to reproduce a race: no `yes > /dev/null &`, `stress`,
or busy loops. Agents can't reliably kill what they start: the cleanup line is
cut off by a tool timeout or the kill command is refused, and the generators run
forever. On October 25, 32 orphaned `yes` processes from one such probe burned
about 7.7 cores for 13 hours and slowed every build on the machine.

Reproduce the ordering instead: add a deliberate delay at the race point (as
above) and remove it before publication, or repeat the test on its own:

```bash
for i in $(seq 20); do cargo test -p loopflow --test integration <test> < /dev/null || echo FAIL; done
```

Orphan Session proofs must launch outside every registered Task checkout; changing
branches in the same directory does not remove checkout membership. Run these
proofs with a canonical `TMPDIR` (on macOS, `TMPDIR=/private/tmp`) so temporary
path symlinks cannot mask that association.

Session association changes also require the full `session_lifecycle_tests` suite:
explicit Task bindings retain membership even from a sibling checkout. Wave label
changes require `DesktopHeadlessTests` alongside `WaveTests` to verify rendered titles.

Fixture Machines must set `LF_HOME` explicitly, including when overriding `HOME`.
Open fixture stores at `$LF_HOME/loopflow.db`; no variable selects another file.

Every integration test binary except the deliberate real-provider `codex_live_smoke` links
`rust/loopflow/tests/support`, whose pre-main initializer removes inherited
`CODEX_HOME` and `CLAUDE_CONFIG_DIR`: agent shells export the developer's real
provider home, and a fixture launch would swap its login there.

Managed-account status fixtures also pin `CLAUDE_CONFIG_DIR` to a disposable
native home: verification observes the native login before polling managed
accounts. Put managed homes below the fixture root so their sibling login locks
cannot collide in the shared temporary directory.

When changing Machine selection, run the affected fixtures with `LF_HOME` unset in
the test runner. CLI fixtures must select their own disposable Machine. Upgrade
proofs open a temporary shared store through the promotion boundary.

Machine command changes also require `cargo test -p loopflow --test global_commands`
to preserve repository-independent commands and explicit Machine isolation.
When changing remote account-selection docs, run `cd website && uv run python
dev.py test -k test_docs_subscriptions_page_owns_account_selection`; the rendered
guide must retain the current invocation, laptop account selection, and target-resident login contract.

`scripts/test.py` and `test_network.py` clear all inherited `LF_*` authority.
The prepared `LOOPFLOW_TEST_LF` candidate survives the network boundary; provider
fixtures pin `LF_BIN` inside it. On macOS, set `TMPDIR=/private/tmp`: Wave/Task fixtures that
register raw `/var/folders` paths otherwise disagree with canonical repository
readers. This affects cron lookup, authored-goal context and exact-Task roadmap
checks, not just orphan Session proofs. The provider harness prepends the selected CLI's parent
to PATH, so an inherited CLI directory containing `claude` can outrank a fixture's
fake provider and launch the real one. A temporary `LF_HOME` alone does not prevent
this. Keep the failed evidence if this occurs, stop the test group, and verify the
fixture under the corrected executable context before completing the suite.
`test_network.py` clears `LF_*` before exec; provider fixtures must pin `LF_BIN`
inside that boundary. Integration `EnvGuard` uses `CARGO_BIN_EXE_lf`, not the
installed CLI directory, so its provider stand-ins remain first on PATH.

Direct source tests retain their own target and the support crate's offline CLI
preparation. Materialized test artifacts embed a disposable source path; execute
them through the wrapper while that source exists.

Default-runtime selection reads the OS account's installation records. Use the
installation harness for default-runtime proofs; never replace the machine's
selection to make tests pass. Graph/Session tests with an explicit experimental
`LF_HOME` and source `LF_BIN` stay within that experiment.

`lf install preflight` and `promote` read the OS account's store and take
its promotion lock; `HOME` and `LF_HOME` do not redirect them. Tests that run
either command are installation proofs: ignored in the regular suite and listed
in `scripts/test_task_installation.py`. On a developer machine they would copy
the live database, without bound while other workers write to it.

For executable-resolution failures, reproduce with the compiled test binary:
unset `LF_BIN` and `CARGO_BIN_EXE_lf`, and use a PATH containing Git but no `lf`.
Verify the repair in that same environment. A pass under a developer's installed
Loopflow can hide the CI failure.
Include direct provider-harness startup tests in this check: even an expected
spawn failure first resolves the conversation's `lf`. Pin a fixture executable
under the environment lock and restore the pin afterward.
Attachment-rejection fixtures must also supply provider version probes and restrict
PATH to their stand-ins: Claude checks `--version` before launching on first input.
A locally installed provider must not make an otherwise incomplete fixture pass.

Changes to terminal provider probes or spawning must run both `session_cli_tests`
and `agent_startup_tests`. An executable that exits with an error still started
and must record its opening; use an absent or non-executable fixture for spawn
failure, with PATH restricted so no installed provider can take its place.

Release repair checks must cover completion before inherited checkout locks close.
Use the public release path with a delayed repair launcher; a terminal Process receipt
does not prove that its process or descendants released their descriptors.

When a subprocess fixture signals readiness with file contents, write a sibling
temporary file and rename it into place after closing it. File existence alone
can expose an empty file between creation and the first write.

### Project readiness fixtures

Task fixtures must create their Wave and Project and select the Project in SQLite
before creating work. YAML binding imports belong to
explicit activation tests; passive status reads do not import them. Keep readiness
fields in inline Swift responses as well as shared DTO fixtures.

Changes to Project selection or planning admission require the full
Rust suite and headless Swift tests, including Task consumers and status readers.
Run the installation harness for migration changes; its released-source proof
must retain Wave identity before projecting accepted Projects.

### Shared identity fixtures

When changing Session activity or Waiting, run `session_cli_tests` and
`world_recipes` (its `--watch` cases) together. Direct SQL fixtures must carry both the attachment
token and the AgentProcess id, matching the production activity writer. Rebuild both SwiftPM and
the Xcode test targets after shared model renames; Foundation types such as
`Foundation.Process` need explicit qualification where names overlap.

Exercise Session fixtures through Rust as well as Swift after ancestry changes.
`wave_id` uses WaveId's UUID encoding; prefixed Task/Project Work IDs are different
types. A Swift String round trip alone cannot prove that a Rust producer accepts
an identity value.

Session fixtures share `AMBIENT_TASK_ENV` in
`rust/loopflow/tests/support/ambient.rs`. Its guard clears inherited control Machine,
captured-input identity/directory, Task authority, and review Session identity under the
suite's environment lock and restores them afterward. Prove isolation from a
live Session without shell-level scrubbing:

```bash
cargo test -p loopflow --test integration session_cli_tests::session_names_are_shared_and_human_names_win
```

Headless provider transport changes also run the research-artifact test
`two_task_research_runs_leave_distinct_uncommitted_artifacts_for_the_next_prompt`
and Linux `planning_reconnect_tests`. Their stand-ins must consume and echo the
stream-input request before reporting success; sleeping and emitting an uncorrelated
print-mode result can race the seed write and fail with a broken pipe.

Headless shutdown checks run on Linux as well as macOS: fast provider exit must
settle its owned lifeline watchdog without mistaking it for foreign work. Include
`launch_nonzero_exit`, bare/default launch, mixed-provider Graph and headless resume;
Session interactivity must not select an individual launch’s process-group policy.
Retained terminal changes also run Linux Clippy, `transport_tests`,
`interactive_graph_keeps_input_and_advances_only_after_each_provider_exits`, and
the disposable installation proof `declared_agent_can_start_another_tasks_graph`.
Assert that attachment returns after execution exits, as well as checking its
output: a spare slave descriptor can keep Linux PTY readers open indefinitely.
Process ancestry assertions follow the recorded chain through transport owners
rather than assuming a fixed number of launch processes.
Terminal-status isolation assertions match the escaped OSC sequence, not its
numeric opcode: ordinary paths and random IDs can contain `7501`. Transport
fixtures include that text in their Home paths to expose false positives.
Changes to AgentProcess launch-mode recording also run the complete
`session_record::runtime::tests` module: native refusal and headless cleanup
fixtures must declare opposite launch modes, independent of Session defaults.
