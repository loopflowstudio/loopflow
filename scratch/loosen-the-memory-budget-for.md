# Gradual Wave memory budgets

Accepted task direction: Jack Heart requested a gradual transition on 2026-09-30
(LOO-362): use a 16,000-token default and curate oversized memories just under
budget by retiring the largest stale sections. Keep total input at 64,000 tokens.
LOO-356's configurable budgets and per-field provenance are present in base
commit `356792067` (#1376).

The change stays in the existing budget resolver and realign skill. Raise the
memory byte default to 128 KiB alongside the token default so the byte cap does
not keep excerpting Product. Preserve overrides, source pointers, reduction
evidence, and total input limits. Align rendered budget feedback and user docs
with gradual curation. No memory files need curation in this implementation.

## Delete — do not maintain

Remove the 8,000-token / 64 KiB memory defaults and broad compression guidance
that encourages rewriting memory toward a small target. No stores, APIs, or
compatibility paths are removed.

## Remaining work

Implementation and gate are complete. Publication and landing belong to
subsequent Flow steps. LOO-356 is already in the branch's base.

Review finding: increasing only the token limit would leave Product excerpted
by the old byte cap. The matching 128 KiB default resolves that mismatch; the
regression retains a 70,000-byte memory whole and checks the 16,000-token boundary.
Explicit override precedence and source preservation retain their existing tests.

Gate review found no additional defects. This branch's `lf context` retains
Product's gathered 14,189 tokens / 66,111 bytes whole; Infrastructure's 23,709
tokens / 112,521 bytes becomes a 13,530-token excerpt. Both report default memory
limits of 16,000 tokens / 128 KiB and input limits of 64,000 tokens / 512 KiB.
Under the former defaults, Product exceeded both memory limits. No memory files
were changed. Prompt goldens establish delivery of gradual curation instructions;
they do not establish an agent's curation judgment on a live memory.

Checks: `cargo nextest run -p loopflow --lib --test context_tests --test golden_prompt --test global_commands -E 'test(engine::context) | test(engine::exec::tests) | test(engine::prompt::) | test(engine::builtins::tests) | binary(context_tests) | binary(golden_prompt) | test(context_budget_preview_reads_authored_wave_without_registration)' --no-fail-fast --build-jobs 4 --test-threads 4` (138 passed); `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `uv run python scripts/check_architecture.py`, `git diff --check` (passed); `uv run pytest python/tests/test_loopflow_skill_alignment.py` (4 passed); `uv run python dev.py test` in `website/` (78 passed, 3 skipped); `target/debug/lf context --wave product --json` and `--wave infrastructure --json` (confirmed measurements above). Full materialized Rust matrix remains CI-owned; the changed-aware runner creates a separate worktree, contrary to this Run's execute-here instruction.
