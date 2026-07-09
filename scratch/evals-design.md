# `lf eval` — is loopflow worth it, provably?

## The trap

The obvious eval is: drive a set of prompts through loopflow and through some
other orchestrator, then judge the relative quality of the outputs. This is
extremely hard, and we are not going to do it. An LLM judge on open-ended code
is noisy and unfalsifiable; arms must start from byte-identical state; runs are
stochastic, so a verdict needs N repeats and the cost multiplies; and any task
drawn from history has its answer sitting in the git log.

## The move

**Don't build a judge. Borrow the one that already exists.**

Every landed change that fixed something came with a test. That test is an
objective, human-authored grader for exactly this task, written by the person
whose bar matters, and it is free. Completion becomes an exit code, not an
opinion.

So, for a commit `C` with parent `P` (the SWE-bench construction, on our own
repos):

1. Split `C`'s diff into **test files** and **code files**.
2. Check out `P` into a throwaway worktree. Apply **only the test patch**.
3. Run the tests. They must fail. That validates the task is real.
4. Give the agent `C`'s *intent* — its title and body — never its diff.
5. Grade: the tests pass. Nothing softer.

The code patch never enters the worktree, so there is nothing to contaminate.
The corpus is real work from real projects, per the KR, and it self-validates:
a task that doesn't reproduce is dropped, not repaired.

## What running it taught us

The judge problem dissolves. An **environment** problem replaces it, and it is
the whole cost of this project.

Validated by hand against manabot `c7e93d3` ("reward: add potential-based
shaping"), a clean 1-test / 3-code split from real work:

- worktree at parent, test patch applied → the test **errored at import**:
  `ModuleNotFoundError: managym._managym`. A compiled Rust extension that does
  not exist in a fresh worktree, and whose prebuilt `.so` is pinned to
  cpython-3.12 while the worktree resolves 3.14.

A naive validator accepts that task. Then *every* arm fails identically, and a
harness that measures nothing looks like it is working. So step 3 above is not
"the tests fail" — it is:

> **The tests must fail for the right reason:** they must run and fail an
> assertion, not error during collection, import, or build.

That single discriminator is what separates an eval from a thermometer in a
freezer. It is also why SWE-bench ships containers.

The same construction against cadenza `9a3d164` ("sync: repair metadata-only
piece storage"), worktree at parent `ba8ec3c`, test patch applied, code patch
withheld:

```
FAILED tests/test_pieces.py::test_create_piece_repairs_idempotent_metadata_only_piece
FAILED tests/test_pieces.py::test_create_piece_fails_when_storage_unavailable_in_prod
2 failed, 11 passed  (exit 1, 0.5s)
```

Two failures, named after the commit's own intent. Eleven passes. That is a
valid task, and it took half a second to prove it.

**The eleven passes are the validator, not the exit code.** A healthy
environment is one where the pre-existing tests still pass; a broken one
(manabot) collects nothing and passes nothing. So:

> **Validation rule.** At `P` with the test patch applied: collection succeeds,
> the pre-existing tests pass, and the new tests fail. Any other shape — a
> collection error, zero passes, or new tests that already pass — drops the
> task.

Exit codes corroborate (pytest: `1` = assertions failed, `2` = collection
error) but they are per-runner and weaker. The pass count is portable.

## Corpus, chosen for the environment tax

Pick first tasks whose tests need no native build:

| repo | test layout | env tax |
|---|---|---|
| cadenza | `server/tests/*.py`, pure Python | low — the place to start |
| loopflow | `rust/loopflow/tests/*.rs` (integration) | low — `cargo test` builds from clean |
| manabot | `tests/**` + compiled `managym` ext | high — needs a pinned build at `P` |
| hootro | — | unknown |

Loopflow's *unit* tests live inline in `#[cfg(test)]` blocks, in the same file
as the code they test, so a file-level test/code split is impossible for them.
Only the `tests/` integration files are harvestable. This is a real constraint
on the corpus, not a bug to fix.

## Arms

Arms differ only in the harness. Same prompt, same grader, same model, same
base commit — or the comparison is a lie.

```yaml
# evals/cadenza-9a3d164.yaml
name: cadenza-metadata-only-piece-storage
repo: ~/src/cadenza
base: 9a3d164^                       # run at the parent
test_patch: patches/9a3d164.test.diff
prompt: sync: repair metadata-only piece storage
check: uv run pytest server/tests/test_sync.py -q
arms:
  - name: loopflow
    command: [lf, code, "{prompt}"]
  - name: bare-claude
    command: [claude, -p, "{prompt}", --output-format, stream-json, --verbose]
```

`{prompt}` substitutes. No other templating — a knob we refuse to add.

Wall-clock is the runner's own timer, always. It includes loopflow's overhead,
and **that overhead is the thing under test** — never subtract it.

## Measurement: one home, not two

Both arms land in `run_events` as terminal run rows, tagged `wave = eval/<task>`
and `flow = <arm>`. A loopflow arm journals itself (the runner mints its
`LF_RUN_ID` and reads the row back); a bare vendor arm does not, so the runner
parses the vendor's own `stream-json` with `engine::stream::StreamParser` and
writes the row. This is why `run_token_usage` was dropped: evidence has one
home, an eval arm is just a run, and `lf trace <run-id>` explains either arm.

## The other eval, which costs nothing

The A/B above answers "does loopflow beat the bare loop." A second, cheaper
question — "is loopflow moving the portfolio" — is answerable from the ledger
with no LLM spend at all: cost and wall-clock per landed PR, per repo, per
month.

Today it cannot be answered, and the reason is adoption, not instrumentation:

| project | last `lf` run | ledger rows |
|---|---|---|
| loopflow | 2026-07-09 | 2720 |
| cadenza | 2026-07-06 | 161 |
| manabot | 2026-05-18 | 0 |
| hootro | 2026-06-20 | 0 |

manabot committed heavily on 2026-07-09 without loopflow. It is not a gap in
the telemetry; it is the live counterfactual. Worth knowing before claiming the
harness moves the portfolio.

The ledger is also only 5 days deep (`run_events` arrived in migration 047,
earliest row 2026-07-04), and its `repo` column is polluted with basenames like
`src`, `tmp.Rf5ZtVARiJ`, `.tmpzt80hK`. Any month-long KR is measuring from
2026-08-04 at the earliest, and needs a repo identity worth grouping by.

## Slices

1. **Harvest + validate.** Given a repo and a commit range, emit candidate
   tasks and keep only those whose tests fail *for the right reason* at `P`.
   Zero LLM spend. This is the asset; the runner is the easy half.
2. **One task, two arms, one table.** Completion / wall-clock / cost / tokens,
   both rows in the ledger. One command, rerunnable forever.
3. **Suite + history.** `lf eval` runs the corpus; `lf eval --since 30d` reads
   the trend from the ledger, so "wins three months running" is a query.

## What this is not

- Not a model benchmark. Arms hold the model fixed and vary the harness.
- Not an LLM judge. The grader is a test someone already wrote.
- Not remote. No results server, ever — the ledger is the store.
