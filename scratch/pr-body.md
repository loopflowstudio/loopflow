Organize PR commands under Task, inspection under Monitor, accounts under Account, and release operations under Repo. Unique shorthand keeps commands such as `lf land`, `lf ps`, and `lf top` available without predecessor aliases. Add work and account overviews with reasons, next actions, and explicit missing evidence; carry access selection through child launches and document first local setup.

Integrate the shared Task-work reader and one-Home store APIs. Monitor observation and Session history use shared Home resolution. Update CLI fixtures and regenerate the command reference to remove retired installation options.

Repair CI failures from bff65e3fee5e3eb48a8864570b4b47df3e7b48a6: graph assertions now expect the canonical `task sync` label, and the cross-checkout managed-review fixture binds its Team and Initiative before Task creation. TESTING.md requires these checks for CLI owner-tree changes.

Repair validation: reproduced all three graph failures and the installation failure before fixing them. `cargo test -p loopflow --lib engine::flow_graph::tests --jobs 4` passed all 7 tests; `uv run python scripts/test_task_installation.py` passed all 6 disposable Linux installation proofs plus its planning migration check; `cargo clippy --all-targets --jobs 4 -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` passed. Full hosted CI remains pending on the repaired head. Installation proofs use simulated providers; they do not establish live-provider lifecycle behavior.

Review: preserve the graph topology and saved-review assertions, reuse the existing planning fixture helper, and change no production behavior for these CI repairs.
