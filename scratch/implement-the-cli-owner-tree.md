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
arrived. This slice now establishes the complete catalog and reviews its
verdicts against callers before implementation.

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
