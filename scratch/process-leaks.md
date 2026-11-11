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

Remaining, at demo: regenerate the report (the committed one is the 20:20
reading: 354, 358 at its own second `ps`, 358 on a separate recount) and show
the `transport_tests` ps diff again. QA walkthrough: `scratch/qa.html`.
Open: `scratch/questions.md` 1a-1d.

Checks:
- `cargo test -p loopflow --test transport_tests`, load 40-60, after dropping
  the per-test `StopOwner`/`ShellOwnerGuard` (Fixture's Drop covers them): 108
  passed, plus `--ignored ssh_shells::` 3 passed; ps diff empty. Earlier, load
  200-330: 6 then 2 `launches(..)` 30 s timeouts, each passed alone.
- 19:02, after dropping four more cleanup-only `fixture.stop` calls:
  `cargo test -p loopflow --test transport_tests -- first_native_launch_keeps_only_valid_desktop_terminal_placement detached_sessions_end_after_their_limit resume_takes_over_an_owner_from_another_build resume_ends_an_unanswering_superseded_owner_of_a_completed_session`:
  4 passed; no new `/tmp/lf-7501-` process. Gate runs the whole suite.
- Regression test fails without the owner fix, passes with it.
- swiftc probe: EOF with `Pipe.uninherited()`, none with `Pipe()`.
- `scripts/test_desktop.sh --skip-build --filter 'ProgramStatusTests|UninheritedPipeTests'`:
  15 passed (after `swift build --build-tests --package-path swift`).

- QA 19:42, load 75-110: `cargo test -p loopflow --test transport_tests` 108
  passed in 107 s; `-- --ignored ssh_shells::` 3 passed. No new process; the
  SSH tests left four `/tmp/lf-7501-*` dirs holding only `screens/` (246 of 305
  such dirs have that shape): an owner writes its screen after its home is gone.
- QA: real `lf -i -a codex` from a host holding a pipe: reader reaches EOF with
  the branch owner alive; installed 0.15.2 owner keeps fd 4. Live Dev app on an
  `lf-world`: Desktop holds 16 pipes, 0 in pane children or owners.
- `uv run python scripts/third_party.py check` exits 0 after `refresh` (the
  `Cargo.lock` change had left `inventory.json` stale).
- `lf task status MTG-269 --select task.state --json` (installed) took 13 min
  once at load 295, in `SqliteStore::task_work`; the audit now gives each `lf`
  read 120 s and names the ones that did not answer.

## Concept review (2026-11-10)

- `Pipe.uninherited()` covers every Desktop `Pipe()` today, but a new `Pipe()`
  reintroduces the leak silently. GhosttyKit forks panes, so the one-place
  fix is not ours. Leave the call-site form; no lint.

- 19:10 pass over `94ebffc67` and `249c21b21`: no design change. `lf process
  rollover --dry-run` read 194 stays: 117 "keeps its lf until it exits", 76
  "native input holds the conversation", 1 Graph. Its reasons answer "can this
  move to the selected lf", so shells and observers all read as the first;
  troubleshooting covers shells in its own paragraph. A "what holds this"
  column in `lf process list` is question 1d, not this Task.
- This Task ships under 1a (behaviour unchanged, documented). Submit clears
  `scratch/`, so the PR body carries 1a-1d and the five held Tasks.

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
  `scratch/questions.md`, which records these five Tasks.
