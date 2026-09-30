# CLI ownership and readiness · LOO-338

Jack Heart requested implementation of every deferred CLI requirement from
LOO-337 on 2026-09-30. Jack Heart’s latest steer supersedes the demo boundary: finish autonomously;
the supervising session ships after the catalog and CLI agree and focused checks pass.
Earlier demo requirements below are historical; they no longer require a wait.

## Complete target

The Clap tree is the sole command catalog. Canonical commands live under their
objects, and navigation derives unique owner omission from that tree. Remove
predecessor parser variants rather than retain aliases. `mon` is derived by unique-prefix resolution, not a fixed alias (Jack Heart, September 30). Account has no short alias. Task delivery also works in an ordinary
repository without a registered Task.

- Task owns PR, `wt`, commit and rebase operations. Repo owns releases.
- Monitor owns the selected monitoring operations. Jack retains ps/top;
  the catalog leaves runs-derived active/watch/detail choices open at demo.
  Its overview joins existing Work, Session and process observations, reporting
  waiting, blocked, active and finished items with reason and next action.
  Missing observation never proves liveness or completion.
- Account owns logins and routing. Its overview exposes usable connections,
  observed capacity and attention per provider/account with LOO-340 live observation by default and explicit cached mode, never
  converting unknown/stale capacity to zero or unlimited.
- Foreground, background and remote launches resolve the access required by the
  selected work before effects. Inherited selection restrictions remain binding;
  a remote child checks destination capabilities, not origin readiness alone.
  Implement this in existing preparation/launch owners, without a second ledger.
- First-project setup leads to a local provider result without a planning account.
  Connected Task creation explains its separate planning prerequisites. Verify the
  actual newcomer path and repair its demonstrated obstacles.
- Reconcile every documented command with the final tree (including Home, Wave
  and Session controls), remove deferrals only when true, and regenerate docs.

LOO-298 owns the Exec/AgentSession/FlowSession model. Its local naming history
includes `a9611a7f3`, `b24b56493`, `99e755e09` and `a000e8d68`; this branch starts
before that cutover. Monitor show must consume Exec or Session identity after
integration, never introduce a new Run command or relabel predecessor records.
No competing model migration belongs here.

## Done when

1. Canonical tree, unique shorthand, typed Flow commands and concrete callers
   agree; removed namespaces have no parser aliases.
2. Both overviews show actual state and truthful gaps with next actions.
3. Background/remote child readiness respects required access and inherited
   restrictions, with behavioral proofs at those launch boundaries.
4. A disposable public-CLI walkthrough produces a first local result, exercises
   monitor and account on real state, and one operation from every owner.
   Label simulated services separately. Help output and fixtures alone cannot
   establish this proof or an autonomous lifecycle.
5. Docs and generated copies describe implemented behavior; Jack reviews at demo.

## Current work after the latest steer

Rebase onto main first; reconcile #1360's deleted service commands and their
arguments. Merge independent duplicate readers C002/C028/C134, updating Desktop,
help, documentation and public-CLI proofs together. Finish the remaining owner
and option cuts and integrate actual Exec/Session readers for Monitor. Jack Heart
removed the demo wait and delegated monitoring keep-or-cut choices; selected
choices are in the catalog's post-rebase section. The supervisor ships only after
the final catalog and CLI match. This pass does not select Flow navigation.

## Slice ledger

The catalog now covers 142 command rows (including root), 440 flags and 95
positionals, with all 677 rows checked against compiled Clap metadata. Public
`help --all` agrees on visible commands (125 output lines, disposable Home,
no Home state written). Ten extra aliases are separately accounted for.
Fresh primary-source research and historical recommendation dispositions are
in `cli-research-20260930.md`. Every command/argument has a verdict in
`cli-command-catalog.md`, with raw extraction and machine-readable judgments.

A new public-CLI counterexample makes reserved-name typed help a prerequisite
for removing `skill show`. Its merge verdict includes that repair. No owner
moves have been applied. The compiled example is read-only extraction tooling.
Formatting and all-target Clippy passed before the catalog checkpoint.

The remaining implementation follows the catalog, starting with the duplicate
skill-inspection path and proven ignored options. Required proof: reserved-name
help must display its body without a provider on PATH; removed options reject
and cached reads retain their behavior. Afterward, all owner moves, overviews,
LOO-298 integration, readiness and live first-result proof remain required. Every target above remains required before
demo readiness; no Task completion or landing follows from a catalog alone.

### Implementation after the catalog checkpoint

Catalog checkpoint: `ad023a72c`. Current slice implements C141 (merge skill show
into typed help), C039/A full and C080/A no-sync deletions, and all ten extra
alias deletions. The reserved-definition escape is preserved only for typed
help; ordinary help delimiters and execution passthrough retain their meaning.
Primary short/long flags remain one option. STYLE.md no longer recommends
restoring uppercase aliases.

The cached-status proof initially failed with `no wave in context` because its
subprocess inherited LF_RUN_DIR from this worker. Branch isolation correctly
cleared the fixture's ambient Wave selection when it detected foreign execution.
The fixture now starts with an empty environment, explicit PATH, HOME, LF_HOME
and LF_DB_PATH before adding its selected Wave. No production authority rule
was weakened. Initial failing command:
`env -u LF_HOME -u LF_DB_PATH -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_RUN_CONTEXT -u LF_FLOW_STEP -u LF_HUMAN_SESSION cargo test -p loopflow --test wave_resolution_tests cached_status_honors_the_shared_resolution_rules`.

Final focused proof:

- `cargo test -p loopflow --test cli_discovery`: 10 passed, including reserved
  typed help, removed-input exit 2, no stdout/runtime writes, and help/discovery.
- `cargo test -p loopflow --test wave_resolution_tests cached_status_honors_the_shared_resolution_rules`:
  1 passed after fixture isolation; seeded state, not live planning.
- `cargo test -p loopflow --lib consolidated_commands_parse_without_old_namespaces`:
  1 passed; removed flags reject and existing commands parse.
- `cargo test -p loopflow --bin lf reorder_args`: 16 passed.
- After updating the synthetic alias example from identity/id to monitor/mon,
  `cargo test -p loopflow --test cli_discovery transitive_lookup_counts_canonical_targets_and_respects_exact_aliases`:
  1 passed; no broad behavioral rerun needed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`
  and `git diff --check`: passed. Generated website docs resynced with
  `uv run python website/dev.py sync-docs --source docs` (21 files; ignored copies).
- `cargo run -p loopflow --example cli_catalog > scratch/cli-catalog-current.json`:
  passed; current 140 commands below root / 437 flags / 94 positional arguments /
  0 extra aliases, versus 141 / 440 / 95 / 10 at baseline. The lost third flag
  is auto-help on the removed skill-show command, not another user option.

Measured production Rust and builtin-instruction diff from `ad023a72c` to this
slice: **+35 / −33 lines**. Excludes test modules and integration tests, generated
files, public docs, style guide and scratch evidence. Command and argument
counts include all hidden surfaces; no reduction is claimed from planned moves.

Review changed the implementation: the initial deletion of skill-show required
preserving reserved definition inspection, and help's ordinary `-- task`
delimiter must not become a definition escape. Both have public-CLI proof.
Deleted uppercase-only reorder tests instead of converting them into duplicates.
Existing ordinary Wave-status fixture now supplies its own environment rather
than weakening source isolation. No new domain owner, state store or alias
registry was introduced. Catalog verdicts remain
requirements for following slices, including the separate retired-op deletion;
no assertion of a completed reorganization, overview, provider journey or demo.

Compression shares one typed-help lookup for escaped and ordinary names, removes
worktree-list's obsolete catch-all pattern, and makes catalog rendering byte-stable.
Proof: all 10 `cli_discovery` tests, repeated catalog rendering, formatting,
all-target Clippy (four jobs), and diff checks passed. Remaining scope is unchanged.

### Slice review · 2026-09-30

Reviewed baseline `a6b1bc3df` through `a2a06bd45`, including catalog verdicts,
parser/dispatch, definition rendering, callers and documentation. **Pass for
this cleanup slice; gap for whole-Task readiness.** No bounded production defect
was found. The shared typed-help reader replaces `SkillCommand::Show` and its
dispatch; ignored `full`/`no_sync` fields and ten extra aliases are removed.
Current caller searches found only negative tests for the removed options;
historical chapter records and baseline catalog evidence remain unchanged.
Definition execution still uses its existing external-subcommand path. No
storage writer, recovery identity, fallback reader or execution authority moved.

Fresh executed proof: `cargo test -p loopflow --test cli_discovery
typed_help_inspects_reserved_definitions_without_launching` passed (1 test),
exercising the compiled CLI in disposable directories with no provider on PATH
and no Home state created. `uv run pytest
python/tests/test_loopflow_skill_alignment.py` passed (4 tests). This establishes
inspection and documentation alignment, not a live provider result. Reuse the
recorded unchanged-code discovery, cached-status, parser, reorder, formatting
and Clippy passes above; no broader suite was rerun.

Checked every baseline command/argument identity against the verdict JSON:
142 commands including root plus 535 arguments, with no missing identities.
Retained extraction counts agree: 141 → 140 commands below root, 440 → 437
flags, 95 → 94 positionals, 10 → 0 extra aliases. The compressed slice from
`ad023a72c` to `a2a06bd45` measures **+40 / −41** production Rust and builtin
instruction lines, excluding test modules/integration tests, catalog tooling,
generated files, docs, style and scratch. The earlier +35/−33 is the
pre-compression measurement.

Convergence: `ad023a72c` is Jack's required catalog checkpoint, not a failed
implementation pass; `5205c8388` replaces skill inspection and removes inputs;
`a2a06bd45` compresses that replacement. There are not two consecutive
implementation passes replacing nothing. Next: move Task PR/worktree/commit/
rebase owners with typed Flow, Desktop, scripts, skills and tests in the same
cut; prove delivery on an ordinary branch without Task registration. Integrate
LOO-298 before monitor identity work. Overviews, local Wave/unlinked work
discovery, child readiness, remaining catalog verdicts, real first-result
walkthrough and Jack's demo remain required. Full Done when claims do not hold,
so this review does not publish, land, complete the Task or choose Flow navigation.

### Task delivery owner implementation · 2026-09-30

Starting revision: `590541b9a`. Switched Task PR/worktree/commit/rebase from root
parser variants to the existing Task owner; CLI and typed Flow dispatch reuse
the original operations. Updated builtin Flow commands, operating guidance,
Desktop review argv, scripts, current documentation and test consumers. Removed
`commit --push`, worktree `--format` (replaced by `--json`) and the retired
`op` parser/rejection shim. Existing PR/commit/rebase shorthand comes from the
Clap tree; no compatibility alias or storage migration was added. Historical
chapter/release/migration bytes and the catalog baseline stay unchanged.

Review found two boundary details worth preserving: saved `pr land` command
bytes must remain readable independently of newly authored `task pr land`, and
rebase outside a repository must retain its actionable error with the canonical
path. The public discovery proof checks the former; the missing-repository
proof checks the latter. A mechanical text replacement briefly treated the
`pr-review` skill as a PR command; review corrected it before verification.

The first `cargo test -p loopflow --bin lf reorder_args --jobs 4` failed 2/16
tests: they bypassed normalization with the former root `commit` input. The
process entry normalizes before reordering. One fixture now uses canonical
input, and the global-Wave/commit-message case exercises normalization then
reordering, preserving the `-m` ownership assertion. The rerun passed 16/16;
no production normalization or authority rule was weakened.

The ordinary-branch public-CLI proof passed with a real temporary Git repository
and local bare remote, synthetic GitHub responses and a browser stub. Commit
changed HEAD without pushing or creating a PR; rebase preview preserved HEAD;
worktree stdout parsed as one JSON array; open produced a draft and publish
made it ready; the store still had no Tasks. This proves ordinary-branch
delivery ownership, not hosted GitHub, provider execution or the required
first-local-result journey.

Focused proof (all passed; no affected-suite/full gate was run):

- `cargo test -p loopflow --test pr_tests task_delivery_works_on_an_ordinary_branch_without_registration --jobs 4`: 1 passed.
- `cargo test -p loopflow --test pr_tests draft_open_stays_draft_until_publish_in_cli_and_flow --jobs 4`: 1 passed, CLI and typed Flow with simulated GitHub/browser.
- `cargo test -p loopflow --test cli_discovery --jobs 4`: 10 passed, including removed options, reserved help, canonical/shorthand equivalence and saved-command bytes.
- `cargo test -p loopflow --lib lf::tests --jobs 4`: 52 passed.
- `cargo test -p loopflow --bin lf reorder_args --jobs 4`: final 16 passed after the two fixture corrections above.
- `cargo test -p loopflow --lib engine::flow_graph::tests --jobs 4`: 5 passed.
- `cargo test -p loopflow --test global_commands missing_repository_and_missing_home_are_distinct --jobs 4`: 1 passed.
- `cargo test -p loopflow --test worktree_tests wt_list_ --jobs 4`: 2 passed, unchanged ordinary reads and explicit sync.
- `cargo test -p loopflow --test golden_prompt --jobs 4`: 1 passed across current prompt goldens.
- `swift test --package-path swift --filter LocalWaveAgentLauncherTests`: 8 passed. Compiler warnings about existing weak variables elsewhere remain unrelated; no rendered app acceptance is claimed.
- `uv run pytest python/tests/test_loopflow_skill_alignment.py -q`: 4 passed.
- `uv run --project website --extra test pytest website/tests/test_readme_index_sync.py -q`: 1 passed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `uv run python scripts/check_architecture.py`, shell syntax and `git diff --check`: passed.
- `uv run python website/dev.py sync-docs --source docs` and `uv run --project website python scripts/render_architecture_html.py`: regenerated website copies and tracked architecture HTML.
- `cargo run -p loopflow --example cli_catalog > scratch/cli-catalog-current.json`: regenerated the compiled extraction. Current counts: **139 commands below root, 17 hidden, 435 flags, 92 positionals, 0 extra aliases**. Baseline: 141 / 18 / 440 / 95 / 10. This pass deletes one namespace, its automatic help flag, two positionals and commit's push flag; moving owners does not remove leaf operations.

Measured production Rust and builtin instruction diff from `590541b9a` to this
pass: **+168 / −218 lines**, excluding test modules/integration tests, catalog
tooling, generated artifacts, docs/style, scratch and the external skill copy.
This is a consumer replacement, not a completed repository reorganization.

Full-design checks remain incomplete: remaining owner moves, both overviews,
LOO-298 integration, local discovery, child readiness and real provider
walkthrough still precede Jack's demo. No publication, landing or Task completion
is implied by this pass.

Compression passes named `RebaseArgs` through CLI/Flow dispatch and removes
redundant commit defaults (29 net Rust lines removed). Rebase parser (1), ordinary
branch delivery (1), discovery (10), formatting, all-target Clippy and diff checks
passed; the compiled catalog is byte-identical. Remaining requirements are unchanged.

### Task delivery owner review · 2026-09-30

Reviewed the active PR diff from `a6b1bc3df` through `db20b06b1`, with the
delivery slice compared against `590541b9a`. **Pass for this slice; gap for
whole-Task readiness.** No bounded production defect found. CLI dispatch,
typed Flow commands, Desktop review argv, builtin instructions and current
documentation use Task delivery owners. The root Pr/Wt/Commit/Rebase parser
variants, retired-op rejection shim, commit push option and worktree format
option are removed. Unique owner omission uses the existing Clap navigation;
saved PR command bytes remain unchanged. No storage writer, Task admission
rule, recovery identity or process authority moved.

Fresh executed evidence:

- `cargo test -p loopflow --test pr_tests task_delivery_works_on_an_ordinary_branch_without_registration --jobs 4`: 1 passed. Real disposable Git repository and bare remote; commit does not push, rebase preview preserves HEAD, worktree stdout is one JSON array, draft open becomes ready on publish, and no Task is created. GitHub and browser responses are simulated; no hosted publication or provider result is proved.
- `cargo test -p loopflow --test cli_discovery command_targets_compose_and_captured_operations_remain_readable --jobs 4`: 1 passed. Canonical authored commands and retained predecessor PR bytes resolve together without rewriting the retained record.
- `git diff --check`: passed. Reuse the recorded unchanged-source parser,
  discovery, typed Flow, Swift, prompt, formatting and Clippy passes above;
  this review adds no code or broader-suite claim.

Measured `590541b9a..db20b06b1`: **+156 / −240 production Rust and builtin
instruction lines**. Excludes test modules/helpers, integration tests, catalog
tooling, generated artifacts, docs/style, scratch and the external skill copy.
The internal commit/push/draft composition still has a release-preparation
caller in `ops/release.rs`; deleting it with the public flag would break that
distinct consumer.

Convergence holds: `5205c8388` replaced duplicate inspection and removed ignored
inputs; `52816c03b` replaced delivery parser/dispatch consumers; `db20b06b1`
compressed their argument forwarding. These are not consecutive passes replacing
nothing. Next implement the remaining Account/Repo/Home/Wave/Session owner cuts
and integrate LOO-298 before Monitor identity readers. Both overviews, local
Wave/unlinked-work discovery, child readiness and the real first-provider-result
walkthrough remain required before Jack Heart's demo. Full Done when claims do
not hold: no publication, landing, Task completion or Flow navigation follows
from this review.

### Account owner and cached overview · 2026-09-30

Starting revision: `f46efdfc6`. Replaced root `Auth`/`AuthCommand` and its
command module with Account; no predecessor alias or short account name.
Bare `lf account` now runs the same cached reader as `account status`;
`account --json` emits its existing report schema. Text retains credential
presence versus verification, routing/cooldown, window age/reset and unknown
capacity, and adds next actions. It does not contact providers or the inherited
broker, decrypt local credentials, or create an absent Home. No account store,
readiness ledger or execution authority was added.

Switched the CLI caller, script argv, builtin guidance, reconnect diagnostics,
current docs and test consumers together. Searches found no Desktop Account
argv to migrate. Auth was not a supported typed Flow mechanical operation;
this slice adds no Flow authentication operation. Provider-native `gh auth` and
`claude auth` remain their providers' commands. Historical Wave/release/migration
records and baseline catalog rows remain dated evidence. Growth memory keeps
Jack Heart's account correction and records the branch's partial progress.

Review changed the work: a mechanical type replacement also renamed the
provider's `AuthCommandInput`; restored that unrelated provider helper. The
initial check failed on an accidental `provider_account_account_status` import;
restored `provider_account_auth_status` before proof. The initial test command
with the superseded filter `cached_auth_leaves_an_absent_home_absent` ran zero
tests and is not counted. The two `account_overview` tests below supply the
required proof. Cached inspection never declares current server acceptance;
unknown local/managed capacity remains unknown rather than zero or unlimited.

Executed proofs (all passed on final production code):

- `cargo test -p loopflow --test auth_tests account_overview --jobs 4`: 2 passed.
  Public CLI, disposable Homes, no tools on PATH. Bare overview and cached
  status JSON agree; missing credential and retained 73% window/reset survive;
  no account mutation or credential directory creation; absent Home stays absent.
- `cargo test -p loopflow --test cli_discovery --jobs 4`: 11 passed, including
  canonical account help, nested shorthand and predecessor-name rejection.
- `cargo test -p loopflow --lib lf::tests --jobs 4`: 52 passed.
- `cargo test -p loopflow --lib account_report_fixture --jobs 4`: 1 passed;
  fixture JSON round-trip, capacity missingness, next actions and readable widths.
- `cargo test -p loopflow --test auth_tests cached_status_keeps_local_evidence --jobs 4`:
  1 passed; synthetic broker socket receives no cached-inspection connection.
- `cargo test -p loopflow --test auth_tests headless_connect_without_a_saved_profile --jobs 4`:
  1 passed; canonical repair command, no registration and no stdin wait.
- `uv run pytest python/tests/test_loopflow_skill_alignment.py -q`: 4 passed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`,
  `uv run python scripts/check_architecture.py`,
  `uv run ruff check scripts/demo_profile_routing.py tests/e2e/linear_oauth.py`,
  `bash -n scripts/bootstrap-cron-host.sh` and `git diff --check`: passed.
- `uv run python website/dev.py sync-docs --source docs` and
  `uv run --project website python scripts/render_architecture_html.py`:
  regenerated website docs and tracked architecture HTML.
- `cargo run -p loopflow --example cli_catalog > scratch/cli-catalog-current.json`:
  regenerated compiled metadata. **139 commands below root, 17 hidden, 436 flags,
  92 positionals, zero extra aliases.** Account adds one overview JSON flag;
  renaming its family removes no leaf operation. Baseline remains
  141 / 18 / 440 / 95 / 10.

Measured `f46efdfc6` to this working implementation: **+104 / −62 production
Rust and builtin instruction lines**. The comparison pairs renamed files and
excludes test modules, test-only source files/helpers, integration tests,
examples/catalog tooling, scripts, generated artifacts, docs, memory and scratch.
Moving the two command files is not counted as wholesale deletion/addition.

These are local public-CLI and fixture proofs, not live authentication,
provider execution, installed acceptance or full-design completion. No affected
suite or full gate ran. Remaining owner cuts are Repo/Home/Wave/Session;
LOO-298 must precede Monitor identity readers. Monitor, local Wave/unlinked-work
discovery, destination readiness preserving restrictions and a real disposable
first-provider-result walkthrough remain required before Jack Heart's demo.
No publication, landing, Task completion or Flow navigation is implied.

Compression after `b13c57d93` routes the overview through Status dispatch,
removes duplicate settings validation and rendering branches (14 net Rust lines),
and deletes the orphaned direction golden whose YAML case was already retired.
Proof passed: `auth_tests` (5), `account_report_fixture` (1), `golden_prompt` (1),
formatting, all-target Clippy (four jobs), and diff checks. Remaining scope is unchanged.

### Account owner review · 2026-09-30

Reviewed the active branch against `a6b1bc3df`, focusing on the Account cut
`f46efdfc6..7b13a99e5`. **Pass for this slice; gap for whole-Task readiness.**
No bounded production defect found. Parser, dispatch, scripts, builtin guidance,
recovery commands and current docs use Account. `Commands::Auth`, `AuthCommand`
and the old command modules are replaced, with no predecessor or account-short
alias. Remaining auth argv belong to native providers; historical migration
comments and negative tests do not provide a competing CLI owner.

Bare Account delegates to Status and its existing read-only store, credential
presence and local metadata readers. There is one transient report, no new
writer or credential authority. Text adds next actions; JSON retains its schema.
Missing credentials, unchecked server acceptance, retained window age/reset and
unknown capacity remain distinct. Cached forwarding reports uninspected identity
without contacting its broker. Account was not a typed Flow operation and has
no Desktop command caller to migrate.

Fresh executed proof:

- `cargo test -p loopflow --test auth_tests account_overview --jobs 4`: 2 passed.
  Real CLI in disposable Homes with no provider tools; overview/status JSON
  agree, stale 73% capacity and missing credentials survive, account rows stay
  unchanged and absent credential/Home directories remain absent.
- `cargo test -p loopflow --test cli_discovery account_has_one_owner_without_predecessor_aliases --jobs 4`:
  1 passed; canonical help and rejected predecessor names, without launch effects.
- `git diff --check`: passed. Reuse the recorded unchanged-source account,
  broker, parser, discovery, golden, formatting and Clippy proofs above. No
  broader suite, live authentication or installed acceptance is claimed.

Measured `f46efdfc6..7b13a99e5`: **+113 / −85 production Rust and builtin
instruction lines**, pairing renamed files and using Git's line diff after
excluding test modules. Excludes test-only files/helpers, integration tests,
examples, scripts, generated artifacts, docs, memory and scratch. The net +28
matches the implementation's +42 followed by compression's −14; rename churn
is not counted as wholesale replacement.

Convergence holds: `52816c03b` switched Task delivery consumers, `b13c57d93`
switched Account consumers, and `7b13a99e5` compressed its shared dispatch and
rendering. These are not two consecutive implementation passes replacing nothing.
Next: finish Repo/Home/Wave/Session owner cuts with caller and canonical-path
proofs; integrate LOO-298 before Monitor's Exec/Session readers. Monitor, authored
Wave/unlinked-work discovery, destination readiness preserving restrictions,
the real disposable first-provider-result walkthrough, remaining catalog/R01–R10
requirements and Jack Heart's demo remain required. Full Done when claims do
not hold; this review does not publish, land, complete the Task or choose Flow
navigation.

### Repo ownership and stop for cull · 2026-09-30

Starting revision `abf449df5`. Replaced root Release/Tokens/Ci parser variants
with Repo children, preserving release runtime capture, checkout source
measurement, and checkout-independent Home CI evidence. Switched CLI and typed
Flow dispatch, scheduled release definition, publisher argv, Desktop source
measurement argv, instructions, recovery hints and docs together. Saved release
command bytes still execute through derived navigation without rewriting them.

Executed proof: `cargo test -p loopflow --test repo_commands --test cli_discovery
--jobs 4` passed 3 operation tests and 12 discovery tests; `cargo test -p loopflow
--lib ci_report_accepts_machine_wide_filters --jobs 4` and the same command with
`scheduled_release_prefers_its_operation_flow_over_the_builtin_skill` each passed
1 test. Real disposable Git/source data and local manifest bumps are exercised;
no release is published. `swift test --package-path swift --filter
repositorySourceMeasurements` passed 1 decoding/argv test with synthetic data
(existing Ghostty linker symbol warnings remain). `uv run pytest
python/tests/test_release_publisher.py -q` passed 6 simulated publisher tests.
Formatting, all-target Clippy (four jobs), architecture inventory, Ruff on changed
Python and diff checks passed. Website copies and architecture HTML were
regenerated. Compiled catalog remains 139 commands below root / 17 hidden /
436 flags / 92 positionals / zero extra aliases. No leaf reduction is claimed.
Production Rust and builtin instructions: **+54 / −55** versus `abf449df5`, using
Git line differences after removing test modules and free test functions;
excludes integration/tests/helpers, scripts, examples, Swift, generated files,
docs and scratch. Review retained CI's ability to inspect from outside Git and
found the scheduled release definition still using predecessor bytes; it now
authors the canonical path while the saved-definition proof retains old bytes.

Jack Heart then stopped further owner moves and requested a concept-based cull
before implementation continues. This checkpoint preserves already-tested work;
it does not establish acceptance of the old verdicts. Jack also directed guides,
skills and agent instructions to use the shortest unique invocation, kept `wt`
as the command name, and required CI to catch documentation shortcut collisions.
The next work is that verdict revision and those corrections. Home/Wave/Session
moves, LOO-298/Monitor, discovery, child readiness, real provider proof and demo
remain required, subject to the revised concept map. No publication, landing,
Task completion or Flow navigation is authorized by this checkpoint.

### Destination-aware cull revision · 2026-09-30

Jack Heart's new direction supersedes the earlier rename-only trajectory. The
catalog audit records current roadmap and seven open PRs, all repository Wave
memories, sibling designs and their contradictions. The correct read used this
Run manifest's runtime with inherited Home; LF_BIN's different-store result was
incomplete. No installed data or sibling checkout was edited.

142 baseline command concepts and 535 argument roles now have dispositions:
48/66/17/11 keep/rename/merge/delete commands, 397/1/90/47 arguments. These are
proposed/adopted targets with explicit open integration/demo choices, not compiled
reductions. Actual counts remain 139 commands below root, 436 flags, 92 positionals
and zero aliases. LOO-298/340 additions require a new integrated catalog before
final counts. Every merge/delete is listed in the rendered catalog.

Source inspection disproved three preliminary cuts: top is explicitly retained
by Jack; saved FlowSession list/show cannot merge into definition help; and
--as is the ordinary execution declaration, not redundant with Task-only launch.
LOO-332 also invalidates adding --no-wait to preserve the old watcher. The final
catalog corrects all four. LOO-334's local-planning scratch differs from Jack's
newer existence direction; no private DB is accepted as current Task truth here.

This pass changes design/tooling only after checkpoint 23c588154 (+0/−0 production
Rust). No behavioral runtime pass or completed cull is claimed. No further owner
move, publication, landing, Task completion or Flow navigation occurred.

### Short commands and wt correction · 2026-09-30

Compared with cull checkpoint `91ed67b9c`. Jack Heart's selected correction is
implemented: canonical `task wt` replaces `task worktree`, no alias; `lf wt`
resolves through owner omission. Parser, CLI/Flow dispatch, operation hints,
fixtures and current reference use that spelling. Guides, README, skills and
operating instructions now omit unnecessary owners. `lf pr publish` keeps pr
because release also publishes; `lf land`, `lf rebase` and `lf wt create` work
without prefixes. Reference/help retain the owner tree. Removed README's stale
Approve/Iterate command examples in favor of returning review feedback.

Unique command prefixes now use the same Clap resolver after exact command
lookup. Hidden callbacks require exact names and never become shortcuts. `mon`
is a derived prefix when Monitor is integrated, demonstrated on the resolver's
synthetic tree; this correction does not introduce the Monitor group. A public
CLI `reb --help` proof exercises real prefix lookup without provider access.

`documented_commands` scans Markdown in docs, builtin skills, exported skills,
README and STYLE through the actual normalizer, including help paths. A deliberate
ambiguous reference example is explicitly marked and must remain ambiguous;
a competing-command fixture proves a formerly unique shortcut fails. This is
an ambiguity guarantee, not execution of every example or proof that pending
preview commands and arbitrary authored definitions exist. The existing public
docs still label the unimplemented target; their full runtime reconciliation
remains required after the cull. STYLE owns the short-command convention.

Focused checks on final production code:

- `cargo test -p loopflow --test documented_commands --test cli_discovery --jobs 4`:
  2 + 12 passed, including reference ambiguity, derived-prefix collision,
  wt/rebase public help, preserved canonical paths and absent-Home inspection.
- `cargo test -p loopflow --test worktree_tests wt_list_ --jobs 4`: 2 passed;
  normal listing preserves canonical checkout, explicit sync owns its update.
- `cargo test -p loopflow --lib consolidated_commands_parse_without_old_namespaces --jobs 4`:
  1 passed; root wt is derived, canonical task wt parses, worktree is absent.
- `cargo test -p loopflow --test golden_prompt --jobs 4`: 1 passed across goldens.
- `uv run pytest python/tests/test_loopflow_skill_alignment.py -q`: 4 passed.
- All-target Clippy (four jobs), formatting and diff checks passed. Catalog
  renderer Ruff passed after splitting overlong literals; the first Ruff attempt
  failed and its shell still checkpointed the design. That failed check was not
  a pass. The initial discovery build overlapped the prefix edit and returned
  None for mon; rerunning after source edits settled passed 12/12. A premature
  catalog read while its generator was compiling saw an empty redirected file;
  the completed generation is valid JSON. These were verification sequencing
  errors, not passing evidence or reasons to weaken resolver expectations.
- Website docs and architecture HTML regenerated. Completed compiled extraction
  remains **139 commands below root, 17 hidden, 436 flags, 92 positionals,
  zero registered aliases**. No count reduction is claimed for restoring wt or
  adding derived-prefix lookup.

Measured production Rust and builtin instructions: **+96 / −86 lines** versus
`91ed67b9c`, excluding cfg/test functions, integration tests, scripts, generated
files, docs/style, external skills and scratch. No domain writer, persistence,
process authority or additional owner move changed in this correction.

Review corrected the old fixed-mon prose, caught the impossible “land resolves
to land” explanation after mechanical shortening, and retained pr on publish
because of the release collision. LOO-340's newly available sibling plan was
also read: live/default and cached proof now exist there; one per-provider
selection covers the whole launch, with no individual step overrides. Source
proof remains separate from live authentication and installed acceptance.

Remaining: integrate destination work and catalog its added/removed Clap nodes;
implement the concept cull with consumer preservation; complete Monitor,
local discovery, child readiness and real first-provider-result proof; reconcile
all preview docs; demonstrate counts and open choices for Jack. No publication,
landing, completed Task or chosen Flow navigation follows from this correction.

Compression after `43fa2059a` removes unused alias handling, table cloning and
the separate root argument-table type; prompt tests check the included guide
instead of duplicating command spellings. Discovery/documentation (13 + 2),
CLI argument handling (30), focused prompt/launch (40), formatting, all-target
Clippy (four jobs) and diff checks passed. Remaining scope is unchanged.

### Short-command slice review · 2026-09-30

Reviewed the active branch against `a6b1bc3df`, focusing on
`91ed67b9c..257b360db` and the repairs below. **Pass after bounded repair;
gap for whole-Task readiness.** `task wt` replaces `task worktree` through
parser, dispatch and callers. Exact commands and unique prefixes share Clap
navigation; hidden callbacks remain exact-only. Removed alias handling, table
cloning and the separate root argument table have no competing replacement.
No persistence writer, execution identity or authority boundary changed.

Review found the documentation scanner skipped wrapped inline commands. After
extending extraction across inline-code newlines, `cargo test -p loopflow
--test documented_commands --jobs 4` failed 1/3 tests: builtin Loopflow guidance
recommended wrapped `lf status`, ambiguous across five commands. Corrected it
to `lf wave status`; the new regression preserves this counterexample. Also
shortened generated Task/CI-repair instructions and the website Task example,
which the earlier Markdown-only pass missed. The scanner proves ambiguity
handling, not validity of every option, external definition or preview command.

Executed final proof, with inherited `LF_*` removed from Rust checks:

- `cargo test -p loopflow --test documented_commands --test cli_discovery --jobs 4`:
  3 + 13 passed. Public CLI help uses disposable directories without providers;
  canonical/short paths agree, ambiguous commands fail without launch effects,
  reserved definitions and captured command bytes remain readable.
- `cargo test -p loopflow --lib ci_fix_arm_preserves_task_completion_and_rotation --jobs 4`
  and `cargo test -p loopflow --lib task_seed_uses_the_current_chapter_plan --jobs 4`:
  1 each passed; local fixture evidence, not hosted repair or Task execution.
- `uv run pytest python/tests/test_loopflow_skill_alignment.py -q`: 4 passed.
  All-target Clippy (four jobs), formatting and diff checks passed. CI's existing
  `cargo nextest run --all` includes the documentation integration tests.

Measured production Rust and builtin instructions: `91ed67b9c..257b360db`
**+115 / −154**; with review repairs **+122 / −161**. Git line differences
exclude top-level test modules/functions/helpers, integration tests, scripts,
examples, external skills, generated files, website content, docs, memory and
scratch. Review alone changes **+7 / −7** counted lines. Parser metadata is
unchanged: retained extraction remains 139 commands below root, 17 hidden,
436 flags, 92 positionals and zero registered aliases.

Convergence holds: `23c588154` replaced Repo dispatch consumers; `43fa2059a`
switched wt callers and documentation to derived navigation; `257b360db`
removed duplicate argument metadata. These are not two consecutive passes
replacing nothing. Next integrate LOO-298/334/340 and re-extract their command
additions before applying the catalog's concept cuts. Monitor, authored Wave
and unlinked-work discovery, child readiness preserving restrictions, the real
first-provider-result walkthrough, preview-doc reconciliation and Jack Heart's
demo remain required. No publication, landing, Task completion or Flow
navigation follows from this review.

### Account integration and duplicate reader removal · 2026-09-30

Starting revision `0667fb0ce`. Read destination branches without editing them:
LOO-298 `a0c919803` retains its unfinished model while extracting independent
fixes; LOO-334 `5a2e54b79` retains a copied-Flow preservation counterexample;
LOO-340 `32b6eef00` has committed live-status and Codex-browser slices. The
read-only open-PR list showed #1296/#1354 and the two extracted #1358/#1359 PRs.
These observations do not establish shipment or authorize installed conversion.

`lf rebase --plan jack-heart/keep-account-status-live-and` selected direct rebase.
`lf rebase --manual jack-heart/keep-account-status-live-and` and four
`lf rebase --continue` calls integrated its committed work here. Conflicts in
Account source, tests and instructions retained LOO-339's identity rejection,
LOO-340's live/default and browser behavior, and this branch's Account spelling.
The local rebase ended at `365142efe`; no push or PR retarget occurred. The copied
sibling scratch plan is removed from this checkout's prompt context; its complete
proof remains at `32b6eef00:scratch/keep-account-status-live-and.md`.

Implemented C045/C051: `lf account [provider]` is the single account reader;
`lf account route` is the single route reader. Removed Status and Show parser
variants and their dispatch, retained mutation children, and moved cached,
details, provider, scope and JSON inputs to the surviving operations. Provider
parsing happens before effects. No alias, account store, credential writer or
execution authority was added. Migrated scripts, reconnect hints, builtin
instructions, guide/reference and tests. There is no Desktop Account caller or
typed Flow account operation. Shared account storage, launch-wide provider bundles
and reset redemption remain LOO-340 integration obligations.

Review found inherited identity/browser recovery strings still naming auth and
remote init examples still using auth/status and route/show. They now name the
surviving commands. Cached usage JSON preserves the expired 73% observation;
text correctly says usage unknown after reset. Rejected identity remains distinct
from service unavailability. Default overview uses existing live verification;
it does not claim live verification for local credentials, whose unavailable
server checks remain explicit. Removed-input tests also reject mixing inspection
flags with a mutation before any state write.

Proof uses inherited LF_* removal and LF_BIN pinned to this checkout's source
binary, with disposable Homes, fake providers and browsers. No source binary
opened installed data; no real login, reset, provider result or publication was
attempted. The first focused pass passed auth_tests (8), cli_discovery (13) and
documented_commands (3). After adding retired-input assertions, discovery (13)
and documentation (3) passed again. Public account proofs cover default refresh,
expired/omitted/unavailable usage, cached broker non-contact, identity mismatch,
duplicate-login refusal, retained credentials and route JSON/state preservation.
A final targeted status_refreshes_by_default_and_cached_preserves_evidence pass
also exercises bare Account text/JSON with no provider filter (1 passed).

Additional focused checks: account_report_fixture (1), provider_auth::codex::tests
(3: matching completion, failure, EOF, cancellation and simulated timeout),
lf::commands::account::account_first_tests (12: browser selection, staged reconnect,
identity rejection and refresh preservation), init_connects_the_distributed_system
(1), and global_commands machine_commands_and_catalog_work_without_git_or_a_repository
(1) passed. Skill alignment (4), changed-script Ruff, shell syntax and architecture
inventory passed. Website docs and architecture HTML regenerated. These are
public-CLI/fixture and static proofs, not a full gate or live acceptance.

Measured production Rust and builtin instruction changes: **+86 / −81** against
rebased `365142efe`; **+1,058 / −437** against pre-integration `0667fb0ce`, including
imported identity/browser work. Counts compare physical lines with difflib after
removing test modules; exclude test-only files, integration tests, examples,
scripts, generated files, docs, Wave memory and scratch. Imported work is not
claimed as newly authored here. Account reader consolidation removes two actual
command leaves; passing operations do not establish whole-Task completion.

Monitor/Exec/Session integration, LOO-334 discovery, LOO-340 remaining ownership
and child selection, other catalog cuts, the real first-local-result walkthrough,
final doc reconciliation and Jack Heart's demo remain required. Open monitor
view cuts remain Jack's demo choices. No landing or Task completion is authorized.

Final static checks: `cargo fmt --all -- --check`,
`cargo clippy --all-targets --jobs 4 -- -D warnings` and `git diff --check` passed.
Final `documented_commands` passed 3 tests after instruction reconciliation.
`cargo run -p loopflow --example cli_catalog` produced valid JSON before replacing
its retained extraction: **137 commands below root, 17 hidden, 433 flags,
92 positionals, zero aliases**, versus pre-slice 139/17/436/92/0 and original
141/18/440/95/10. The three fewer flag entries are two removed help flags and
one duplicate JSON option. --cached replaces --verify without adding a second
live toggle. These are intermediate implemented counts, not final demo counts.

Compression after `7285df603` shares managed-provider parsing with routing,
retains typed provider errors and removes redundant async dispatch (19 net Rust
lines removed). Isolated `auth_tests` (8) and Account-first tests (12), formatting,
all-target Clippy (four jobs) and diff checks passed. CLI counts and remaining
scope are unchanged; providers and browsers in these proofs are fixtures.

### Account integration slice review · 2026-09-30

Reviewed the active branch against `a6b1bc3df`, reusing the preceding slice
reviews and focusing on `365142efe..1d7d5aba3`, its imported identity/browser
paths, and the complete Account consumer cut. **Pass for this slice; gap for
whole-Task readiness.** No bounded production defect found. Bare Account owns
live/default and explicit cached inspection; bare route owns route inspection.
`AccountCommand::Status`, `RouteCommand::Show`, their dispatch and current
callers are removed. Searches found no surviving predecessor Account calls;
native provider auth commands and historical migration text remain legitimate.
Shared provider parsing replaces the duplicate parser without a new domain
owner, fallback reader or storage writer. Existing identity, credential and
window writers retain their separate evidence responsibilities.

Fresh executed proof, with inherited `LF_*` removed and `LF_BIN` pinned to this
checkout's `target/debug/lf`:

- `cargo test -p loopflow --test auth_tests status_refreshes_by_default_and_cached_preserves_evidence --jobs 4`:
  1 passed. Public CLI text/JSON refresh, cached reads without provider access,
  missing/expired/omitted capacity, unavailable observation and read-only route
  inspection preserve their evidence and stored state.
- `cargo test -p loopflow --test cli_discovery removed_options_and_aliases_report_usage_errors_without_effects --jobs 4`:
  1 passed. Removed status/show/verify inputs and mixed inspection/mutation
  arguments reject with exit 2 before effects.
- `git diff --check`: passed. Reuse the recorded unchanged-source identity,
  browser/cancellation, absent-Home, broker, discovery, documentation, formatting
  and all-target Clippy passes. No broader suite was rerun.

These proofs use disposable Homes and synthetic providers/browsers, not live
OAuth, installed acceptance or a first local provider result. Measured
`365142efe..1d7d5aba3`: **+94 / −108 production Rust and builtin instruction
lines**, using physical-line difflib comparison after excluding test modules.
Compression alone (`7285df603..1d7d5aba3`) is **+14 / −33**. Excludes integration
tests, test-only files, examples, scripts, generated files, docs, memory and
scratch; imported work already present at `365142efe` is outside this count.
Retained compiled counts remain 137 commands below root, 433 flags, 92
positionals and zero aliases; this review changes no parser metadata.

Convergence holds: the preceding short-command cut switched wt and guidance
consumers; `7285df603` replaces two Account reader leaves; `1d7d5aba3` compresses
their implementation. These are not two implementation passes replacing nothing.
Next integrate the outstanding LOO-298/334/340 destinations and extract their
actual Clap additions before the dependent concept cuts. Monitor, authored
Wave/unlinked-work discovery, destination child readiness with inherited account
restrictions, the real first-provider-result walkthrough, remaining catalog and
R01–R10 requirements, final documentation and Jack Heart's demo remain required.
Open Monitor view choices remain for Jack. Whole-design Done when claims do not
hold; this review does not publish, land, complete the Task or choose navigation.

### Task changed-file projection merge · 2026-09-30

Starting revision `1735f4e94`. Refreshed committed destination evidence:
LOO-298 `c7fcd31ec` records landed independent PRs #1358/#1359 but leaves its
Exec/Session/FlowSession cut, import and configured acceptance on #1296.
LOO-334 `2d63d6488` retains a failed copied-Flow continuation proof: selected
development bytes refuse the preserved production directory after both Flow
comparisons pass. LOO-340 remains the already-integrated `32b6eef00`. These are
local committed-source observations; this pass performs no provider refresh,
integration rebase, sibling edit or installed-store access. The required model
integration is still unavailable under its preservation contract, so this pass
implements the independent concept merge named in the preceding direction.

C101 now merges into C102. `task diff --files` owns changed-path inventory;
ordinary Diff retains patches, optional path and stdin draft comparison. Removed
TaskCommand::Changes and its separate dispatch selection. Desktop's taskChanges
reader invokes Diff's files projection and decodes its existing DTO. Updated
current docs and code comments; remaining predecessor strings outside scratch
are the negative test and historical release notes. No typed Flow mechanical
Task-files operation exists to migrate. No alias or parallel model was added.

Focused proof, with inherited LF_* cleared and LF_BIN pinned to this checkout's
source binary for Rust checks:

- `cargo test -p loopflow --test task_diff_tests --test cli_discovery --test documented_commands --jobs 4`:
  1 + 13 + 3 passed. Public CLI on real disposable Git data preserves recorded,
  head and pinned comparison bases; committed rename paths; staged, unstaged and
  untracked states; patch rename evidence; draft non-mutation and file revision;
  and revision-checked save. Removed Changes and incompatible files/path/draft
  inputs exit 2 with empty stdout and no Home state creation. No planning/provider
  service is contacted; Task registration is a fixture, not live discovery.
- `swift test --package-path swift --filter taskWorkspaceQueriesDecode`: 1 passed,
  proving native reader decoding through the new argv. Existing Ghostty linker
  symbol warnings remain; no rendered desktop acceptance is claimed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`
  and `git diff --check`: passed. No affected-suite or full gate ran.
- `uv run python website/dev.py sync-docs --source docs`: regenerated 21 website
  copies. No architecture page source changed.
- `cargo run -p loopflow --example cli_catalog --jobs 4`: valid JSON captured before
  replacing the retained extraction. Counts **137 → 136 commands below root,
  433 → 431 flags, 92 → 91 positionals, zero aliases**, with 17 hidden commands.
  New --files replaces three flag entries on Changes (base, JSON, help); Diff
  already owns its issue positional. Catalog verdict and argument rows updated.

Review retained separate TaskChangesSnapshot and TaskDiffSnapshot wire shapes:
these are consumed projections of one comparison, not competing storage owners.
It also retained the no-lifecycle file dispatch and prohibited meaningless
files-plus-draft combinations at parsing, before stdin reads. The underlying
file/revision functions and DTO fields stay unchanged. Measured against
`1735f4e94`: **+16 / −17 production Rust lines**, plus **+1 / −1 Swift consumer
line**. Counts use physical-line difflib comparison excluding cfg(test) modules,
integration tests, scripts/examples, docs/generated output and scratch.

Whole-design Done when remains unmet: LOO-298/334 integration, the remaining
LOO-340 store/provider-bundle work, Monitor, discovery, destination child readiness,
other catalog cuts and the live first-provider-result walkthrough still precede
Jack Heart's demo. Preserve all explicit demo choices and public deferral labels
until their proofs hold. This slice does not publish, land, complete the Task or
choose Flow navigation.

Compression after `ac882b020` removes two parser-shape tests (63 net test lines):
one still required the deleted Account status leaf and failed on reproduction;
the other duplicated the public Task-file workflow. Route scope conflicts remain
in public CLI coverage. Isolated `lf::tests` (50), `cli_discovery` (13),
`task_diff_tests` (1), formatting, all-target Clippy (four jobs) and diff checks
passed. Production behavior, compiled command counts and remaining scope are unchanged.

### Task changed-file slice review · 2026-09-30

Reviewed `1735f4e94..1be96f83c` against the Task directive, catalog and full
target, reusing the preceding reviews of the active diff from `a6b1bc3df`.
**Pass for this slice; gap for whole-Task readiness.** No bounded production
defect found. `TaskCommand::Changes` and its dispatch selection are removed;
Desktop's file inventory now calls `task diff --files`. Current callers and
documentation agree. The predecessor spelling remains only in the rejection
test and historical evidence. Files and patches retain their consumed DTOs,
shared placement/base resolution and existing revision/save implementation.
File access still bypasses lifecycle reconciliation and reads recorded placement;
it does not establish LOO-334's future Linear/Git discovery contract. No new
storage writer, fallback reader, alias or execution authority was introduced.

Fresh executed proof, with inherited `LF_*` removed and `LF_BIN` pinned to this
checkout's `target/debug/lf`:

- `cargo test -p loopflow --test task_diff_tests --jobs 4`: 1 passed. The public
  CLI preserves recorded/head/pinned bases, rename identity, staged/unstaged/
  untracked paths, draft non-mutation and revision-checked saves on real disposable
  Git data. Task registration is a fixture; no planning or provider is contacted.
- `cargo test -p loopflow --test cli_discovery removed_options_and_aliases_report_usage_errors_without_effects --jobs 4`:
  1 passed. Retired Changes, incompatible files/path/draft inputs and conflicting
  Account route scopes return exit 2, empty stdout and no Home state creation.
- `git diff --check`: passed. Reuse the recorded unchanged-source Swift decoding,
  documentation, parser, formatting and all-target Clippy passes. No rendered
  Desktop, live provider, installed acceptance or broader-suite pass is claimed.

Measured `1735f4e94..1be96f83c`: **+16 / −17 production Rust lines**, plus
**+1 / −1 Swift consumer line**, using physical-line difflib comparison and
excluding test modules, integration tests, examples/scripts, docs/generated
output and scratch. Compression changes tests only; this review changes evidence
only. Retained compiled counts remain **136 commands below root, 431 flags,
91 positionals, zero aliases** (17 hidden commands).

Convergence holds: `7285df603` replaced duplicate Account readers; `ac882b020`
replaces the Changes leaf and Desktop caller; `1be96f83c` removes obsolete tests.
These are not two consecutive implementation passes replacing nothing. Next
refresh committed LOO-298/334/340 evidence and integrate coherent destinations
with saved-execution preservation before dependent Monitor/discovery cuts. Keep
the copied-Flow counterexample unresolved until proved. Remaining catalog and
R01–R10 work, account ownership/provider bundles, destination child readiness,
the real first-provider-result walkthrough, final docs/counts and Jack Heart's
demo remain required. Preserve open Monitor choices. Whole-design Done when
does not hold; this review does not publish, land, complete the Task or select
Flow navigation.

### Skill catalog concept merge · 2026-09-30

Starting revision `3dd912634`. Refreshed committed dependency evidence without
editing sibling worktrees: LOO-298 remains `c7fcd31ec` and LOO-340 remains the
integrated `32b6eef00`. LOO-334 advanced to `9fea0552f`; its copy-provenance
slice still records public continuation failing at `guard_development_database`
when selected development bytes reach the original production directory. Its
required proof and succession policy remain unresolved. No model integration,
installed-store access, provider request or isolation change was attempted.

Implemented the preceding direction's independent C140 merge. `list skill
[namespace] [--json]` is the single skill inventory. Removed SkillCommand::List
and its inspection dispatch; namespace entries now advertise List, and current
reference examples follow that invocation. Searches found no concrete Desktop,
script, builtin-skill or typed Flow caller of the retired catalog leaf. Existing
Entry JSON, ordering, namespace membership and source precedence use the same
reader. Saved Flow inspection and definition execution retain their owners.

Removing the verb also makes list an ordinary explicit skill name. Without that
skill, `skill list` fails lookup; with it, explicit selection and typed help use
the authored skill despite the same-named Flow. The delimiter still works. No
retired-name guard or alias was added. Review caught stale documentation showing
unnecessary skill escapes; the example now demonstrates Flow's actual collision.
This is definition selection/inspection proof, not a provider execution claim.

Focused proof used inherited LF_* removal and LF_BIN pinned to this checkout's
source binary. The first `cargo test -p loopflow --test cli_discovery --test
documented_commands --jobs 4` passed 13 discovery tests and failed the new test's
assumed exit 1; the actual missing-skill lookup exits 2. A disposable public CLI
reproduction confirmed `skill not found: list`, empty stdout and no Home created.
Corrected the assertion, preserving production exit behavior. Cargo did not run
the documentation target after the initial discovery failure.

Final checks:

- `cargo test -p loopflow --test cli_discovery --test documented_commands --jobs 4`:
  **14 + 3 passed**. Real CLI with disposable definitions and no providers on
  PATH; scoped JSON equals the corresponding mixed-catalog entries, text retains
  names/invocations, nested skills remain visible, retired lookup and reserved
  skill selection preserve their boundaries, and inspection writes no Home.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`
  and `git diff --check`: passed. No affected-suite or full gate ran.
- `uv run python website/dev.py sync-docs --source docs`: regenerated 21 copies.
- `cargo run -p loopflow --example cli_catalog --jobs 4`: validated JSON before
  replacing the retained extraction. **136 → 135 commands below root,
  431 → 429 flags, 91 → 90 positionals, zero aliases**, 17 hidden commands.
  The removed flags are duplicate JSON/help; namespace uses List's path input.
  Updated C140/A530–532 in the existing verdict source and rendered catalog.

Measured against `3dd912634`: **+3 / −19 production Rust lines**, using physical
line difflib comparison of the three changed source files before cfg(test).
Excludes integration tests, docs, generated copies and scratch. The switched
consumer is the emitted namespace invocation plus public catalog documentation;
its predecessor parser and duplicate dispatch are gone. No persistence writer,
credential authority or execution model changed.

Whole-design Done when remains unmet. LOO-298/334 integration, remaining LOO-340
account ownership/provider selection, Monitor, discovery, destination child
readiness, remaining catalog/R01–R10 requirements and a real first-provider
result still precede final documentation/counts and Jack Heart's demo. C134
requires typed help to prove invalid Flow/review compilation fails nonzero before
its validation leaf can be removed; saved FlowSession inventory/detail remain
distinct. Keep the copied-Flow counterexample and all open demo choices. This
slice does not publish, land, complete the Task or choose Flow navigation.

Compression after `f63912dc3` consolidates duplicate reserved-skill fixtures and
help assertions, separates namespace discovery, and shares catalog JSON decoding.
Isolated `cli_discovery` (14), formatting, all-target Clippy (four jobs) and diff
checks passed. Production behavior, compiled counts and remaining scope are unchanged.

### Main rebase and independent cull · 2026-09-30

Rebased `f91ec1731` onto main `3dc89bc9a` with `lf rebase --manual` and
Loopflow-owned continuations, ending at `7164a0d81`. Conflicts retained main's
service deletion and this branch's ownership/cull changes. No push occurred.
The catalog drops nine service command rows and four daemon artifact inputs;
its baseline JSON remains unchanged.

C002/C028/C134 now merge User name, PR status and Flow validate into Home User,
bare PR and typed Flow help. Desktop User argv, short guides and canonical
reference moved together. Flow help expands and validates review IDs before
printing; the retired validate name becomes available for authored Flows.
A011–A016 consolidate context into `--diff files|patch|both|none` and browser
capability into `--chrome on|off`. Omission still inherits config; no dual parser
or alias is retained. Native provider flags and lf-prompt inputs are separate
interfaces and unchanged.

Executed focused proofs with inherited LF_* cleared and LF_BIN pinned locally:
CLI discovery 15, documented commands 3, User CLI 2, ordinary-branch delivery 1,
no-repo machine commands 1, init instruction contract 1, assembled-context
choice/inheritance 1. The latter assembles real temporary Git changes under both
config defaults and checks file bodies, patches and browser capability for every
choice without launching a provider. PR/GitHub and browser effects are simulated;
User and context tests use real disposable Git data. Skill alignment passed 4.

Two counterexamples were corrected: duplicate review IDs report “not unique,”
not the new test's assumed “duplicate”; documentation checking found a stale
retired-op compatibility row left by the rebase. Neither initial failure was a
pass. Final discovery/documentation reruns passed. Architecture inventory passed.
Final formatting, all-target Clippy (four jobs), and diff checks passed.
Website docs and architecture HTML regenerated. Compiled counts are 123 commands
below root / 15 hidden / 393 flags / 82 positionals / zero aliases.

Review retained one config translation at the existing prompt owner and removed
the unused toggle helper. It left native browser flags and prompt-tool flags
intact. Monitor still requires real Exec/Session owners; current Run records are
not renamed. Refreshed LOO-298 `25548d567` and LOO-334 `1fd99a284` contracts replace
older dependency assumptions. `lf rebase --plan 25548d567` reports direct rebase
of 28 commits, touching 268 files; no integration has yet been applied. The full
catalog/CLI agreement and remaining requirements are not complete.
