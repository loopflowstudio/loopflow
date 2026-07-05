# Rebase Efficiency: Placement, Classification, And Workflow Telemetry

Shipped. The design and rationale now live in the code and in
`wave/systems/MEMORY.md` (Shipped entry, dotted-root gotcha, follow-ups, and the
"How to judge rebase efficiency" metrics). What remains here is how a reviewer
exercises the change.

## Try it

Deterministic rebase on a disposable branch:

```bash
lf op rebase --plan            # classify without touching git
lf op rebase                   # execute the chosen strategy
```

Expected on a scratch-only / stale-empty branch — no agent launches:

```text
class: scratch_only
strategy: reset_to_base
agent_launched: false

Stashed scratch/
Reset branch to origin/main
Restored scratch/
```

Placement (ancestry through the planner, not raw `git checkout -b`):

```bash
lf op wt create demo-parent
cd ../loopflow.demo-parent
lf op wt create child          # stacked child: <schema-demo-parent>.child
lf op wt create --main child   # root branch off origin/main
lf op wt create api.v2         # rejected: dots reserved for ancestry
```

Telemetry (ignored `.lf/tmp/`, no diffs or secrets):

```bash
tail -n 5 .lf/tmp/metrics/ops.jsonl   # strategy, stack_parent, agent_launched, duration
```

## Validation

Passed at gate:

```bash
cargo fmt --check
cargo nextest run --all
cargo clippy -- -D warnings
uv run pytest python/tests/test_install_script.py
cd website && uv run python dev.py test
tests/e2e/test_rebase_efficiency.sh
cargo run -q -p loopflow --bin lf -- op rebase --plan
cargo run -q -p loopflow --bin lf -- op wt create --plan gate-doc-smoke
```

Before land, confirm the installed `lf`, source binary, local-bin shim, docs,
and prompt guidance all agree on `lf op wt create --stack [PARENT]`, `--main`,
`--fork`, and `--plan` — command-surface drift was the review's sharpest finding.

## Done when

- `lf op rebase --plan` classifies stale-empty, scratch-only, protected, and
  stack-parent cases deterministically.
- `lf op rebase` resets disposable branches without launching an agent; `scratch/`
  survives via stash/restore.
- `lf op wt create` plans through the placement engine; `a.b.c -> a.b` ancestry
  holds; user segments with `.` are rejected; `--main` is the root escape.
- scratch stash and ops telemetry use ignored `.lf/tmp/` paths and record
  strategy decisions without raw diffs.
- E2E (`tests/e2e/test_rebase_efficiency.sh`) covers the demo path.
