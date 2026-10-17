# Conversation opening demo — October 17, 2026

Jack Heart invoked demo for [LOO-499](loo-499.md) and
[PR #1585](https://github.com/loopflowstudio/loopflow/pull/1585), commit
`59ce70830126b2e27615e3ae5e7c5456f1c71e17`. Review is in progress; no user
acceptance or merge approval has been given.

## Setup and observed evidence

The installed CLI reports 0.14.5 and predates the PR. The demo uses the built
candidate at `target/debug/lf`, a disposable repository and a separate data
directory under `/private/tmp/lf-499-demo-mygvdqdg`. The wrapper
`/private/tmp/lf-499-demo-mygvdqdg/bin/lf` removes inherited LF variables and pins
the candidate and demo data directory. No installed database is opened by it.
Repository configuration selects the real Codex provider; no stand-in or mock
provider is used. Provider launch/authentication is not yet observed.

The cmux workspace `workspace:1000070040`, titled `LOO-499 Connect demo`, is open
with a shell using that wrapper. Its initial instructions guide Jack Heart to
connect, retain the phrase “amber bicycle,” exit and resume to verify history.
The repository guide limits activity to this demo; no Tasks or Flows should start.

Observed by the reviewing agent:

- In an unassociated linked worktree, `lf connect --prepare --json` exits 1,
  explaining the missing work conversation and offering ordinary lf, repository
  connection and exact Session connection.
- With no registry yet, `lf connect --explain --json` reports `Local registry is
  absent` instead of a future repository destination. This is not proof of
  first-creation failure; evaluate the experience alongside the actual launch.
- Defect: the unassociated-worktree error suggests `lf --repo repo connect` for
  this temporary repository. Running that suggestion with `--explain --json`
  resolves `/Users/jack/src/repo`, whose canonical repository is
  `/Users/jack/src/manabot`, rather than the demo repository. No provider was
  launched there. Proposed repair: emit the canonical repository's shell-quoted
  absolute path, and test the printed command from an arbitrary repository
  location with a same-basename repository under the configured root.

## Remaining interactive review

1. Jack Heart opens the first repository conversation with `lf connect` and
   observes the matching repository Session skill.
2. Enter a recognizable phrase, exit to the shell and use `lf resume`; verify
   conversation identity and recalled history, not merely a successful process.
3. Keep that client open and use `lf connect --fresh` in a second demo shell.
   Verify a distinct conversation appears while the original remains usable.
4. Check subsequent connect selects the new primary, and exact-ID opening still
   selects the predecessor. Cover Task/Wave targets and ticket casing afterward.

No native provider behavior or user judgment is yet established. Record Jack
Heart's observations separately from agent interpretation. Resolve demonstrated
defects before recommending acceptance; this demo starts no further Flow.

## Final polish and shipping direction

Jack Heart authorized shipping and landing LOO-499 after review on October 17.
The recovery-hint defect above is repaired: the command uses the canonical
absolute path through shlex. The contextual connect/resume/fresh regression
executes the printed hint with a same-name repository under the lookup root
and passes; formatting and Clippy pass. Live-provider acceptance is still not
established by that test. Gate owns the remaining automated affected checks.
