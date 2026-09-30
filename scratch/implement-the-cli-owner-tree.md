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

Jack expanded scope before owner implementation: catalog every command,
subcommand and option, including hidden/internal surfaces, from Clap itself.
For every row retain the current canonical path, owner, purpose, actual caller
citations, overlap and a keep/rename/merge/delete verdict. Commit this catalog
before implementing owner changes. Then implement all verdicts across parser,
dispatch, docs, skills, Desktop and tests. Demo includes before/after command
and option counts. Absence of a repository caller alone does not establish
absence of public users; inspect defaults and positional inputs too.

The earlier in-progress owner edits were restored to HEAD when this direction
arrived. The catalog prerequisite is complete at `ad023a72c`; the first culling
slice below is verified. The next implementation slice should switch accepted
object owners and their consumers end to end. Do not repeat the baseline audit
or treat this first cleanup as convergence. Integrate/coordinate LOO-298 before
monitor's Exec/Session reader cut. The broader overviews, first local result,
child readiness and every remaining catalog verdict still block demo readiness.

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
