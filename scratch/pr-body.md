## Try it!

```bash
lf op rebase --plan
lf op wt create --plan child
lf op wt create --main root-demo
lf op wt create child --stack
lf op wt create api.v2
```

`lf op rebase --plan` prints the branch class and strategy without changing git.
`lf op wt create --plan child` shows whether the command will create a root,
create a stack child, check out an existing branch, or reuse an existing
worktree. `api.v2` fails fast because dots are reserved for stack ancestry.

Validation run:

```bash
cargo fmt --check
cargo nextest run --all
cargo clippy -- -D warnings
uv run pytest python/tests/test_install_script.py
cd website && uv run python dev.py test
tests/e2e/test_rebase_efficiency.sh
```

## Intent

Make `lf op` own branch placement and rebase strategy decisions so avoidable
long rebases become deterministic reset or parent-rebase paths instead of agent
work.

## Assumptions

Branch ancestry is encoded by dotted branch names for stacked work.
`scratch/` is portable working context and can be copied aside during disposable
reset paths. Existing root branch schemas remain valid, even if they include
dots.

## Key decisions

Placement planning lives in `engine/worktrees`, not in `lfd` or CLI glue.
`lf op rebase` plans before mutation and only resets unprotected disposable
work. Rebase classification uses merge-base diffing so upstream-only drift does
not look like local authored work. Scratch restore is a simple directory copy,
not a patch merge.

## Not included

Normal `lf <flow-or-step> --stack|--fork|--dispatch` placement flags are left
for a follow-up. `lf op land` behavior is unchanged.
