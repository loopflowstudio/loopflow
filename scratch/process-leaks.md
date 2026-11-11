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

Fixed: `transport::start` marks every descriptor above stderr close-on-exec
for the detached owner; `Pipe.uninherited()` (Loopflow library, so Desktop and
`WaveOrigin` share it) for every Desktop child pipe; transport `Fixture` Drop.

Checks:
- `cargo test -p loopflow --test transport_tests`: 108 passed, 8 ignored; no new
  process from `target/`, `/tmp/lf-7501-*` or stand-ins afterwards (ps diff).
- Compress, load average 200-330: full suite twice, 6 then 2 different
  timeouts (`launches(..)` 30 s deadline); each failure passed alone.
- Regression test fails with the owner fix removed (verified), passes with it.
- `Pipe.uninherited()` standalone swiftc probe: EOF with it, none with `Pipe()`.
  `UninheritedPipeTests` itself runs at gate (`--swift`).
