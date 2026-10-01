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

Implementation is complete. Gate owns broader acceptance; publication and landing
belong to subsequent Flow steps. LOO-356 is already in the branch's base.

Review finding: increasing only the token limit would leave Product excerpted
by the old byte cap. The matching 128 KiB default resolves that mismatch; the
regression retains a 70,000-byte memory whole and checks the 16,000-token boundary.
Explicit override precedence and source preservation retain their existing tests.

Checks: `cargo test -p loopflow --lib engine::context_budget::tests` (4 passed); `cargo test -p loopflow --test golden_prompt` (passed); `cargo fmt --check` and `git diff --check` (passed).
