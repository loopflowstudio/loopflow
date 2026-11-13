# LOO-869 plan

`selections::work_in` reads Task state (`task_details::states_in`), published PRs
(`project_adoption::published_in`) and owners (`task_ownership::owners_in`) once
for the repository's Task list instead of three queries per Task. This covers
`plans`, `read_dashboard` and `resolve_selection`, which all go through
`work_in`. `published_in` filters `historical=0`, so the partial index
`idx_task_prs_current` serves it.

Remaining:
- Gate.
- Measure the win in-process: the brief's baseline is about 2,250 statements and
  180–220 ms warm for a 750-Task repository on a store copy. QA (scratch/qa.html)
  measured only whole-CLI debug wall time at load ~68: 751 Tasks, `desktop view list`
  min 378→299 ms, median 551→440 ms; `view show query:qa` min 322→266 ms, median 717→558 ms.
  Output was identical to the base build. A quiet-Machine release number is still missing.

QA: scratch/qa.html (real CLI and served subscription; nothing broken) → done.
Check: `target/debug/deps/loopflow-* store::sqlite::selections::` → 22 passed (implement).
On 2026-11-12 an outside SIGSTOP repeatedly stopped test binaries here and in other
worktrees (state `T`, even under `setsid` with no terminal; source not found in
this repo or `~/.lf/logs/lf.log`). The run passed after resuming its own PID 10 times.
Gate may see the same thing.
