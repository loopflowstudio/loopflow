# Open questions

- **Row speed target.** Brief: "Each decision is checked once, in-process, in
  well under 1 s". Promotion rows measured ~0.4 s each when adopting a
  checkout (git: worktree + `prepare` fetch/merge-base), under 0.1 s when
  refusing or binding; grouped by family, nextest tests take 1.3–3.3 s
  (load average 545–600). Choices: (a) accept per-row cost and 1–3 s family
  tests, as `tests/README.md` now says; (b) one row per nextest test, ~8→~20
  tests each paying fixture setup; (c) cut `prepare`'s foreground fetch, which
  also speeds people's `lf task create --session` (~0.7 s of git). Taking (a)
  until decided.
