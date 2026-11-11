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

Remaining, at demo: regenerate the report (the committed one is the 17:30
reading) and compare its totals with a fresh `ps`; show the `transport_tests`
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

Next implement: the report's "Owners with no open Session" group hides what
it is. Read 19:10: all nine (PIDs 8687 8933 24677 32670 55749 88963 89413
89840 92273) are Desktop shells (`session serve --terminal -- /bin/sh -c
# Runs only on creation…`), 26-32 h old, ppid 1, `/bin/zsh -l` alive, 0 viewers,
rollover "keeps its lf until it exits". `lsof -p 8687` also shows fd 4 on
`~/.lf/run/desktop/com.loopflow.mac.sock`, inherited from Desktop.

- `process_audit.py`: classify a shell owner by its command line; name the
  group "Desktop shells with no viewer"; drop "or a conversation that ended".
  Per shell, show its Task and that Task's state.
- The transport README ends a Desktop shell on Close or fifteen minutes after
  its Task is completed and released; Hide and Desktop exit keep it. A shell
  whose Task is done, abandoned or gone and that is hours old is an owner
  lifetime bug (outcome 3): find why its close never ran, fix it with a test.
  If every one belongs to an unfinished Task, say that in the report row and
  add a shell row to the troubleshooting table (stopping it loses the shell's
  jobs and environment; its saved screen stays).
