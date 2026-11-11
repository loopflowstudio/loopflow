# LOO-823 process leaks

Report: `python3 -I scratch/process_audit.py` writes `scratch/process-audit.html`
(read-only; it opens `loopflow.db` with `mode=ro` for shell receipts).

Readings, 2026-11-10 ~17:30, 343 lf processes:

- 113 `session observe-status`, every one with its Desktop client gone or
  replaced. `lsof` on PID 2046: its stdin pipe's write end (Desktop fd 43) was
  held by 15 pane processes (owners, zsh, codex, git), so EOF never came.
- 24 from `/tmp/lf-7501-*`: `transport_tests` owners and stand-ins up to 8 days
  old; the fixture had no Drop. 297 `/tmp/lf-7501-*` dirs remain from them.
- Owners: 78 held by `native input` (any unacknowledged byte: arrows, Esc,
  an edit), 1 by a running Graph, 9 Desktop shells (below); rollover's dry run
  moved 0.

- `__codex-observe` (50), `__context-block` (23): all beside a live parent.
- 17 demo/Dev-app (LOO-672 Redemo owners, Dev-app observers) from
  `~/.lf/demos/` and `/tmp`; no script here started them.
- 18:40: no `lf` from any other test fixture's home or a `target/` path; the
  `/tmp/lf-7501-*` ones are all 3+ days old.

Since `ab682d71f`, `observe-status` is one write per reading; a resident one
is from an older Desktop.

Remaining, at demo: regenerate the report (the committed one is the 18:58
reading, 356 processes) and compare its totals with a fresh `ps`; show the `transport_tests`
ps diff again. Open: `scratch/questions.md` 1a-1d.

Checks:
- `cargo test -p loopflow --test transport_tests`, load 40-60, after dropping
  the per-test `StopOwner`/`ShellOwnerGuard` (Fixture's Drop covers them): 108
  passed, plus `--ignored ssh_shells::` 3 passed; ps diff empty. Earlier, load
  200-330: 6 then 2 `launches(..)` 30 s timeouts, each passed alone.
- Regression test fails without the owner fix, passes with it.
- swiftc probe: EOF with `Pipe.uninherited()`, none with `Pipe()`.
- `scripts/test_desktop.sh --skip-build --filter 'ProgramStatusTests|UninheritedPipeTests'`:
  15 passed (after `swift build --build-tests --package-path swift`).

## Concept review (2026-11-10)

- `Pipe.uninherited()` covers every Desktop `Pipe()` today, but a new `Pipe()`
  reintroduces the leak silently. GhosttyKit forks panes, so the one-place
  fix is not ours. Leave the call-site form; no lint.
- Open: which input holds an owner (`scratch/questions.md`, 1a-1d).

## Desktop shells (read 2026-11-10 18:55)

The nine owners rollover reads as "no open Session" are unviewed Desktop
shells; the report lists them under `desktop shell` with Task and state from
`desktop_shells`, `task_retirements` and `lf task status`.

- 8933 (LOO-434 active), 88963 (LOO-774 shipping), 92273 (MTG-269 active):
  unfinished Tasks; a hidden shell runs until Close.
- 32670 (LOO-775), 55749 and 89413 (LOO-776), 89840 (LOO-812), 24677
  (MTG-252), 8687 (MTG-268): Task done, `close_requested=0`. The close path
  never started: `retirement::advance` stops at `require_settled_work` because
  the Task's interactive owner has `OwnerRetention::NativeInput`, so the
  retirement stays `waiting` and `request_retirement` is never reached. The
  sweep itself runs (`com.loopflow.cleanup`; `Task cleanup pending` lines in
  `logs/lf.log` through 18:53). No shell-specific bug: it is the input hold in
  `scratch/questions.md`, which now records these five Tasks. No code changed.
