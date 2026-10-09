# LOO-418 post-sync gate

Accepted decisions and complete acceptance remain at
`7d111c125:scratch/make-a-task-up-to.md` and its `scratch/questions.md`.
The previous gate's failures and recoveries remain at
`63565b5d3:scratch/make-a-task-up-to.md`. This pass reviews the current branch
against active PR base `461577746ff3e0862bc7c015d784ee0da247ff16`.

Review found that the user guide said to repeat filings without explaining that
the default obligation key retries the first filing. It now requires a distinct
`--key` for each additional obligation and explains unchanged retry identity even
with a changed title. Existing filing tests cover both behaviors. Completion,
Workflow position, execution and follow-through remain independently represented;
the current Product memory agrees with that contract.

The aggregate gate failed its Python population: four checkout-pruning cases
read the installed Home's older schema, and one benchmark could not enter its
own sandbox inside the runner's sandbox. The benchmark passed separately under
its own read-only sandbox. The four pruning cases remain deferred to Rust CI's
isolated account, as documented in TESTING.md; the installed Home was not changed.
A supplemental disposable-account attempt stopped before container creation:
the UV image pull stalled in its credential helper. This does not invalidate
the separate installation harness's successful populated upgrades and CLI proofs.
No second whole-plan pass is claimed. Logs: `.lf/tmp/gate-loo418*` and
`.lf/tmp/gate/run-6892/`.

Remaining acceptance: configured native stacked delivery and dated follow-up
return, provider continuity, and Jack Heart's complete lifecycle demo. Headless
fixtures do not prove those outcomes or Product's chapter KRs. No publication,
landing, Task completion or live Home migration occurred in this pass.

Authored memory and scratch fit their budgets. The supplied generated goal
inventory exceeded its 16,000-token budget; its omitted inventory was inspected
from the supplied complete local source. No directive, inventory evidence or
limit was changed to conceal that generated overflow.

Check: `uv run python scripts/test.py --base 461577746ff3e0862bc7c015d784ee0da247ff16 --reuse-passing` — Rust 2,363 passed/18 skipped, Desktop build and 409 headless tests, website 76, architecture, formatting, Clippy and Swift boundaries passed; Python 403 passed/5 failed, with `uv run --no-sync pytest python/tests/test_desktop_performance.py::test_checkout_observation_preserves_read_boundary_and_detects_changes` passing separately and four installed-Home cases deferred to CI; `uv run python scripts/test_task_installation.py` passed two populated upgrades and seven CLI proofs; `git diff --check` passed. Preceding sync proof: `cargo test -p loopflow --lib ops::task::tests::task_decision_preserves_unknown_history_and_live_process_protection -- --exact` passed (1 test).
