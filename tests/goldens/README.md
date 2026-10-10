# Prompt goldens

`*.yaml` cases in, `*.md` snapshots out. `golden_prompts_match_python` (in
`rust/loopflow/tests/golden_prompt.rs`) asserts the current prompt assembly
reproduces each `.md` exactly.

```bash
uv run python tests/goldens/update_goldens.py   # regenerate all .md snapshots
```

Run this whenever you edit an embedded prompt source — `LOOPFLOW.md`, anything
under `rust/loopflow/src/builtins/` (surfaces, skills, directions), or the
assembly code in `rust/loopflow/src/prompt/`. Skipping it leaves the golden stale and fails
`rust-test` in CI. `*.actual.md` files are test debris; don't commit them.
