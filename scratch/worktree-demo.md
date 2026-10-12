# Worktree placement demo

2026-10-12 — Jack Heart requested autonomous verification and a successful demo,
without asking Jack to perform the fixture steps. All four cases passed through
the built `lf` CLI. This establishes filesystem behavior, not integration with
the third-party applications.

| Existing layout | Observed new checkout |
| --- | --- |
| Conductor city folders `warsaw`, `tokyo` on unrelated branch names | `conductor/workspaces/app/next` |
| Herdr full-branch slugs `feature-login`, `fix-search` | `.herdr/worktrees/app/next` |
| Worktrunk siblings `app.feature-login`, `app.fix-search` | `app.next` |
| Claude `login`, `search` on `worktree-login`, `worktree-search` | `app/.claude/worktrees/next` |

For each disposable repo, Git seeded two foreign-tool checkouts and a local bare
remote. `lf wt create next --plan` reported the expected path and created nothing.
`lf wt create next` created it. Repeating plan reported `use_existing_worktree`;
repeating creation reused the same directory. Four worktree registrations remained
(primary, two examples, new checkout). `lf wt switch <existing-full-branch>` selected
the original foreign checkout. Uncommitted files in both existing and new worktrees
remained byte-for-byte intact, as did main's commit. No lf configuration was supplied
or generated. HOME and lf state were isolated from the actual repository.

Replay: `uv run python scratch/demo-worktree-placement.py` after building the CLI.
[Normalized CLI output](worktree-demo.json) records all plan/create/reuse/switch
results. The script deletes only its disposable fixtures on completion.
[Current design](how-will-lf-break.md) retains broader boundaries and open questions.

Jack's feedback establishes autonomous demonstration as sufficient for this
mechanical behavior; it does not establish acceptance of third-party lifecycle
integration or the remaining design choices.

Check: `uv run python scratch/demo-worktree-placement.py` — all four layouts passed,
including path, plan-only, reuse, existing-branch selection and dirty-file preservation.
