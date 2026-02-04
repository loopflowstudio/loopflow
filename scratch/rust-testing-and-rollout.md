# Rust Lf Parity Testing and Rollout

## Problem

Rust `lf` is close to parity with Python but we lack a rigorous, repeatable way to prove it and a concrete path to switch users over without breaking UX invariants. The users who benefit are anyone running `lf` daily; we need confidence that prompts, flows, and outputs are identical before the Rust CLI becomes primary.

## Approach

Establish a three-layer parity harness (prompt parity, golden context, end-to-end workflows) and make Rust the primary CLI with a scoped Python fallback. The harness is deterministic, offline, and versioned in-repo. We treat Python output as the temporary source of truth until Rust hits the parity bar, then freeze goldens and move to Rust-owned generation.

1) Prompt parity tests
- Build fixtures as small, versioned git repos under `tests/parity/fixtures/`.
- Run Python `lf` and Rust `lf` with identical flags on each fixture.
- Normalize the prompt output (paths, timestamps, ordering) and compare byte-for-byte.
- Fail with a diff that includes a minimal, normalized repro.

2) Golden context tests
- Encode context inputs as YAML and expected output as Markdown.
- Provide a single `update_goldens.py` script that uses Python `lf` to generate expected files.
- Rust tests compare actual vs expected and write `.actual.md` on mismatch.

3) End-to-end workflows
- Add shell-based smoke tests that exercise `lf ops` on a temp repo.
- Use a mock GitHub provider (or env flag) to avoid network access.
- Keep E2E tests minimal: create worktree, commit, rebase conflict detection, local land.

4) Rollout: Rust primary with Python fallback
- Rust CLI runs first; on explicit "not implemented" errors it shells out to Python.
- The fallback is temporary and tracked in tests (assert fallback path is unused once parity reaches target).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Only Rust unit tests | Fast, local | Doesn’t prove UX invariants or prompt parity |
| Only golden files | Deterministic | Doesn’t validate full CLI behavior or flags |
| Python remains primary | Low risk | Blocks Phase 2 and slows iteration |

## Key decisions

- Treat prompt parity as the gate for Rust primary. This follows **"UX invariants: prompts, flows, directions, and artifact paths must not change"**.
- Keep tests offline and deterministic to uphold **"Control/execution isolation"** and CI stability.
- Use Python as the temporary source of truth while we validate **"Protocol first"** parity; once parity passes, freeze goldens and switch generators to Rust.
- Scope fallback to explicit not-implemented paths to align with **"Rust-first implementation"** without hiding regressions.

## Scope

- In scope: parity fixtures, prompt normalization, golden tests, E2E smoke tests, temporary Rust->Python fallback.
- Out of scope: full lfd parity, distribution packaging, auth, container/K8s execution.

## Done when

- `uv run pytest tests/parity/test_prompt_parity.py` passes on all fixtures.
- `cargo test -p loopflow-engine golden_tests` passes with no `.actual.md` leftovers.
- `./tests/e2e/test_full_cycle.sh` and `./tests/e2e/test_rebase_conflict.sh` pass in CI.
- Rust CLI runs `lf run` and `lf ops` without fallback on the parity fixture set.
