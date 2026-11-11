# LOO-823 process leaks

Report: `python3 -I scratch/process_audit.py` writes `scratch/process-audit.html`
(read-only; ps, lsof, `lf session list`, `lf process list`,
`lf process rollover --dry-run`).

Readings, 2026-11-10 ~17:30, 343 lf processes:

- 113 `session observe-status`, every one with its Desktop client gone or
  replaced. `lsof` on PID 2046: its stdin pipe's write end (Desktop fd 43) was
  held by 15 pane processes (owners, zsh, codex, git), so EOF never came.
- 24 from `/tmp/lf-7501-*`: `transport_tests` owners and stand-ins up to 8 days
  old; the fixture had no Drop. 297 `/tmp/lf-7501-*` dirs remain from them.
- Owners: 78 held by `native input`, 1 by a running Graph, 9 with no open
  Session; rollover's dry run moved 0.

- `__codex-observe` (50) and `__context-block` (23): every one beside a live
  parent; none stale.
- 17 demo and Dev-app processes: LOO-672 Redemo owners with providers alive 5
  days, plus Dev-app observers. Demo runtimes live under `~/.lf/demos/` and
  `/tmp`; no demo script in this repository started them, so nothing here to fix.

Fixed: `transport::start` marks every descriptor above stderr close-on-exec
for the detached owner; `Pipe.uninherited()` (Loopflow library, so Desktop and
`WaveOrigin` share it) for every Desktop child pipe; transport `Fixture` Drop.
After the sync onto `ab682d71f`, `observe-status` is one write per reading, so
a resident observer now only comes from an older Desktop;
`docs/troubleshooting.md` says so.

Remaining: demo (run `transport_tests`, ps diff, open a fresh report) and the
count check (report totals against a fresh `ps`), both at demo/gate.

Checks:
- `cargo test -p loopflow --test transport_tests` after the reaper move, load ~80:
  107 passed, 1 read timeout (`unanswering_superseded_owner_…`, passed alone);
  ps diff empty for `target/` and `/tmp/lf-7501-*`.
- Compress, load average 200-330: full suite twice, 6 then 2 different
  timeouts (`launches(..)` 30 s deadline); each failure passed alone.
- Regression test fails with the owner fix removed (verified), passes with it.
- `Pipe.uninherited()` standalone swiftc probe: EOF with it, none with `Pipe()`.
  `UninheritedPipeTests` itself runs at gate (`--swift`).
- Sync onto `ab682d71f`: kept server-backed Program Status delivery, which
  removes the observer child and its pipe; other pipe-inheritance fixes remain.
  `scripts/test_desktop.sh --skip-build --filter 'ProgramStatusTests|UninheritedPipeTests'`
  — 15 passed after `swift build --build-tests --package-path swift`; no rerun needed for sync.

## Concept review (2026-11-10)

Next implement direction, settled by the design:

- One reaper: done. `loopflow_test_support::end_recorded_processes(home, lf)`;
  `transport_tests`' `Fixture::drop` calls it and the tests README names it.
- `Pipe.uninherited()` covers every Desktop `Pipe()` today, but a new `Pipe()`
  reintroduces the leak silently. Panes are forked by GhosttyKit, so the
  one-place fix (pane children start with only their PTY) is not ours to make.
  Leave the call-site form; no lint.

Open product choice (returned Blocked): which input holds an owner. 78 of ~100
resident owners are held by `native input`, and `Uncertain` clears only through
`clear_owner_input`, so one stray key holds an owner until someone resumes it.
Counterexample to "arrows aren't a draft": Up in Claude recalls the previous
prompt into the editor, which is a draft on screen.
