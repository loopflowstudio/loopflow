# LOO-966 verify (merge 40b1d88c19, PR #1975)

- Checkout tree == origin/main (`git diff --stat HEAD origin/main` empty).
- `cargo test -p loopflow --test integration -- sync_tests::sync_onto_an_open_task sync_tests::publish_keeps_the_prs_base sync_tests::stack` → 10 passed (stack via `lf sync INF-123`, bare sync follows parent, publish twice sends no `--base`, disagreement printed with both commands, detach/restack).
- LOO-959 record on main's `lf task status --json`: stack_parent {identifier LOO-953, pr 1974, branch jack/loo-953-…}; text: `GitHub #1972 … stacked on LOO-953 (#1974, …)`; `gh pr view 1972` base = jack/loo-953-…. Record and PR agree.
- Post-merge push CI for 40b1d88: "CI" success; "macOS on main" still in progress at verify time (carries the Desktop headless view tests); merge_group CI success. Desktop label not looked at in the running app.
