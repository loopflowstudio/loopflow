# LOO-869 plan

`selections::work_in` reads Task state (`task_details::states_in`), published PRs
(`project_adoption::published_in`) and owners (`task_ownership::owners_in`) once
for the repository's Task list instead of three queries per Task. This covers
`plans`, `read_dashboard` and `resolve_selection`, which all go through
`work_in`. `published_in` filters `historical=0`, so the partial index
`idx_task_prs_current` serves it.

Remaining:
- Measure the win in-process: the brief's baseline is about 2,250 statements and
  180–220 ms warm for a 750-Task repository on a store copy. QA (scratch/qa.html)
  measured only whole-CLI debug wall time at load ~68: 751 Tasks, `desktop view list`
  min 378→299 ms, median 551→440 ms; `view show query:qa` min 322→266 ms, median 717→558 ms.
  Output was identical to the base build. A quiet-Machine release number is still missing.

QA: scratch/qa.html (real CLI and served subscription; nothing broken) → done.
Gate (2026-11-12, HEAD 4b4be68e59, merged with origin/main): `scripts/test.py` stopped at
resource preflight (15 s `du` deadline on uv cache, Swift builds and `~/.lf/runs`; 60.6 GiB free,
32 GiB floor), so its plan ran by hand: architecture ok, test inventory ok, `cargo fmt --check` ok,
`cargo build --bin lf` ok, nextest `test(store::sqlite::selections) | test(store::sqlite::project_adoption::)
| test(store::sqlite::task_details::) | test(store::sqlite::task_ownership::) | test(lf::commands::desktop::)
| test(documented_commands::)` → 48 passed, `cargo clippy --all-targets -- -D warnings` ok. CI owns the full matrix.
An outside SIGSTOP stopped this run's test binaries again (state `T`, parent nextest); a loop
resumed them 14 times.
