# Open questions

## Outstanding: rebase onto origin/main

The branch is 1 commit behind `origin/main`. The earlier blocker (a second
`codex exec` agent holding this worktree with uncommitted edits) is gone: the
process has exited and the tree is clean. The rebase can now run.

```bash
lf rebase
```

`origin/main` is ahead by `601881cd waves: reshape roster into wave/project/task
ontology`, plus `3ddf006c install.py: drop stale --global from lf op sync-skills`
— which touches `docs/lfop.md`, the file this branch renamed to `docs/ops.md`.
That rename/edit collision is the one conflict worth reading carefully. This
branch removes the `--global` flag path from `install.py` entirely, so main's
correction likely no longer applies.

For the rest: files central to this branch's intent (the `lf op` → first-class
command sweep — `lf/mod.rs`, `bin/lf.rs`, `ops/*`, goldens, builtin skill
prompts) keep this branch's version. Files main touched incidentally take
main's.

## Latent: `op: rebase --plan` in a flow silently does nothing

`execute_flow_ops` (`src/ops/flow.rs`) returns `Ok(())` for a `rebase --plan`
flow item without planning or printing anything — the flag exists so the *CLI*
can dry-run, and the flow path has nowhere to print a plan. A flow step that
reports success while doing nothing is a footgun.

No builtin flow uses it (the only `op:` payloads in-tree are `pr open`,
`pr land`, `pr land --create-pr`, and `release run patch`), so nothing is
broken today. Left as-is because turning the no-op into an error is a behavior
change, not a reduction. Assumption: a future pass makes `--plan` an
unsupported flow op rather than a silent success.

## Untested: the two new exec-door denials

`wave_exec_verdict` (`src/wave/server.rs`) grew `Deny` arms for `auth`,
`sync-skills`, and `task`. Only `auth` is covered by
`wave_exec_policy_rejects_dangerous_verbs`. Adding `sync-skills` and `task` to
that list is one line each; skipped here because this pass only removes code.
