# LOO-823 process leaks

Report: `python3 -I scratch/process_audit.py` writes `scratch/process-audit.html`
(read-only).

Readings, 2026-11-10 ~17:30, 343 lf processes:

- 113 `session observe-status`, every one with its Desktop client gone or
  replaced. `lsof` on PID 2046: its stdin pipe's write end (Desktop fd 43) was
  held by 15 pane processes (owners, zsh, codex, git), so EOF never came.
- 24 from `/tmp/lf-7501-*`: `transport_tests` owners and stand-ins up to 8 days
  old; the fixture had no Drop. 297 `/tmp/lf-7501-*` dirs remain from them.
- Owners: 78 held by `native input` (any unacknowledged byte: arrows, Esc,
  an edit), 1 by a running Graph, 9 with no open Session; rollover's dry run
  moved 0.

- `__codex-observe` (50), `__context-block` (23): all beside a live parent.
- 17 demo/Dev-app (LOO-672 Redemo owners, Dev-app observers) from
  `~/.lf/demos/` and `/tmp`; no script here started them.
- 18:40: no `lf` from any other test fixture's home or a `target/` path; the
  `/tmp/lf-7501-*` ones are all 3+ days old.

Since `ab682d71f`, `observe-status` is one write per reading; a resident one
is from an older Desktop.

Remaining, at demo/gate: `transport_tests` with a ps diff, a fresh report,
its totals against a fresh `ps`.

Checks:
- `cargo test -p loopflow --test transport_tests`, load ~80: 107 passed, 1 read
  timeout (`unanswering_superseded_owner_…`, passed alone); ps diff empty for
  `target/` and `/tmp/lf-7501-*`. Load 200-330: 6 then 2 `launches(..)` 30 s
  timeouts, each passed alone.
- Regression test fails without the owner fix, passes with it.
- swiftc probe: EOF with `Pipe.uninherited()`, none with `Pipe()`.
- `scripts/test_desktop.sh --skip-build --filter 'ProgramStatusTests|UninheritedPipeTests'`:
  15 passed (after `swift build --build-tests --package-path swift`).

## Concept review (2026-11-10)

- `Pipe.uninherited()` covers every Desktop `Pipe()` today, but a new `Pipe()`
  reintroduces the leak silently. GhosttyKit forks panes, so the one-place
  fix is not ours. Leave the call-site form; no lint.
- Open: which input holds an owner (`scratch/questions.md`, 1a-1d).
