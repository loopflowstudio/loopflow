# LOO-418 post-sync gate

Accepted design, decisions, prior failures and review authorization remain at
`7d111c125:scratch/make-a-task-up-to.md` and `7d111c125:scratch/questions.md`.
This pass compares against active PR base `3e1e6245c` after the #1512 sync.
It does not publish, land, complete the Task or migrate the installed Home.

Review corrected three stale delivery claims: continuation disposition no longer
exists; waited landing can repair CI itself; unanswered turns alone no longer
block checkout work. Recorded live/unknown Processes still do. Product memory
now reflects the shared post-sync execution check; no child Product memories
exist in this checkout. TESTING.md records the installed-Home isolation trap.

The changed-aware runner's Python population exposed four checkout-pruning reads
of the installed Home's older schema and two macOS sandbox restrictions. The
checkout suite passes under a disposable OS account with the candidate built by
the installation harness. The benchmark cases pass without the outer sandbox
(the checkout-observation case still applies its own read-only sandbox).
These recoveries do not turn the initial runner failure into a whole-plan pass.
Logs are `.lf/tmp/gate-loo418*`; the runner's retained phase evidence is
`.lf/tmp/gate/run-17866/`.

The Rust population passed 2,362 tests and failed one successful-cmux fixture
on a missing title file. The unchanged fixture then passed alone in 20.29 s.
Its success assertions depend on each shell command completing inside the
production one-second host deadline; the failed run did not retain the cause
of the missing file. Removed those flaky scenarios rather than adding retries
or changing production timing. The fixture retains Task-first and taskless
provider names, reconnect, OSC output and missing/failing/timed-out host cases.
Successful live cmux renaming is not established by this gate.

Remaining acceptance: configured native stacked delivery and dated follow-up
return, live provider continuity, and Jack Heart's complete lifecycle demo.
Headless fixtures establish neither those results nor Product's chapter KRs.
Historical uncertain export stays pending but locally usable. Unseen Linear
concurrency remains the accepted best-effort limit, not an atomic-write claim.

Context: authored memory and scratch fit their limits. The generated launch
inventory remains over 18,000 tokens against 16,000; its complete supplied
source was inspected. No Task directive, inventory evidence or limit was changed
to conceal that generated overflow.

Check: `uv run python scripts/test.py --base 3e1e6245c --reuse-passing` retained Python 402 passed/6 failed and Rust 2,362 passed/1 failed/18 skipped; isolated checkout suite 19/19 and focused sandbox cases 3/3 passed, materialized repaired terminal-title test 1/1 passed; `scripts/test_task_installation.py` passed two populated upgrades and seven CLI proofs; Desktop build/headless 409 tests, website 76, architecture, Swift boundaries, final `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings` and `git diff --check` passed. Full CI and configured native acceptance remain separate; no second whole-plan pass claimed. The supplied `scratch/sync-proof.md` retains the preceding focused sync result.
