# LOO-532: Desktop performance on built worlds

Jack Heart asked (2026-10-20) for two follow-ups to lf-world (#1612). He reviews
this before it lands.

## 1. Stub provider + native fixture on one-task — implemented

- `world::STUB_CLAUDE` is written to `<world>/bin/claude`; `env.json` gains
  `PATH` with that `bin/` first. `--resume <id>` reads the world transcript and
  prints `resumed <id>: <n> lines`, then `reply <id>: <line>` per input until
  `quit`/EOF; `--session-id` starts an echoing conversation. Transcripts are
  never written (live-Session Tasks LOO-384/402/416/480 may want turns recorded;
  not built).
- `World::create` drops inherited `LF_*` except `LF_BIN`: a builder launched
  from an lf Session otherwise failed admission ("capture does not belong to a
  recorded Session").
- `desktop_performance.py` builds `one-task` with the `lf-world` beside
  `--cli`/`--lf` and reads Task/Session/transcript back through `lf`. Its own
  builder (git init, notes.txt, sh stub, `lf resume`, `task create`, `session
  bind`) is deleted, and so is the scratch repository in verify/terminal-feedback.
  verify-fixture's copied Machine is a second world (`LF_DESKTOP_TASK_REPO`).
- Repairs the opt-in fixture needed (they had rotted on main):
  - sandbox `PROVIDER` still named `bin/codex`;
  - Session transport sockets live in `/tmp/lf-<uid>-<digest>`; the policy now
    allows that one directory (`TRANSPORT`);
  - `session attach --if-free` prints `null`; the fixture's JSON rewrite now
    passes it through;
  - since #1605 `SessionsStore.select` only attaches; the reopen tests `resume`.
- Files tests use `README.md` (the world checkout's file) instead of notes.txt.

## 2. Large recipe — deferred

Jack Heart cancelled the large recipe for now (2026-10-20) after measurement:
per-Task planning writes grow with the plan, so 2048 Tasks takes hours.
Follow-up LOO-541 owns the optimization and the numbers.

## Delete — do not maintain

- `_seed_native_task`/`_create_native_task`, the sh codex/claude stub and
  `PERF_NATIVE_ID`/`PERF_TRANSCRIPT` in `desktop_performance.py` (done).

## Remaining

- Check results: `verify-fixture` (unmounted) 5/5 passed; `world_recipes` 2/2;
  `test_desktop_performance.py` 51 passed; clippy clean; Swift test build ok.

- Mounted journeys (`run`, `verify-fixture --mounted`) not run here: they open
  windows on the host display. Gate/demo owns them.
