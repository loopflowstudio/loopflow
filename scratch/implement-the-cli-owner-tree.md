# CLI ownership and readiness · LOO-338

Jack Heart requested implementation of every deferred CLI requirement from
LOO-337 on 2026-09-30. This branch ships as one PR and waits at demo for Jack;
no landing is authorized.

## Complete target

The Clap tree is the sole command catalog. Canonical commands live under their
objects, and navigation derives unique owner omission from that tree. Remove
predecessor parser variants rather than retain aliases. Fixed `mon` is the documented monitor spelling. Account has no short alias. Task delivery also works in an ordinary
repository without a registered Task.

- Task owns PR, worktree, commit and rebase operations. Repo owns releases.
- Monitor owns activity, ps, top, show, usage, list, active, replay and prune.
  Its overview joins existing Work, Session and process observations, reporting
  waiting, blocked, active and finished items with reason and next action.
  Missing observation never proves liveness or completion.
- Account owns logins and routing. Its overview exposes usable connections,
  observed capacity and attention per provider/account without refreshing secrets
  or converting unknown/stale capacity to zero or unlimited.
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

## This slice

Move Account ownership and its overview end to end from `f46efdfc6`.
Replace the auth parser/dispatch owner, migrate current caller argv, recovery
commands, skills and docs, and expose bare `lf account` through the existing
cached status reader. Preserve the JSON schema and credential/capacity
missingness; add actionable text without refreshing credentials. No account
alias, store, credential reader or launch authority is introduced.

The complete target remains required. Repo, Home, Wave and Session moves,
LOO-298 integration before Monitor, local discovery, child readiness, a real
first-provider-result walkthrough and Jack's demo remain unfinished. This
slice cannot establish whole-Task readiness or authorize landing.

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
