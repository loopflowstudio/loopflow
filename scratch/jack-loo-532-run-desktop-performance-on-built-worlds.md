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

## 2. Large recipe — measured, stopped for Jack's decision (2026-10-20)

Targets: 256 and 2048 started Tasks with checkouts, a Session on every other
(DesktopPerformanceTests) or every Task (WorkspacePerformanceTests).
Debug `lf-world`, M-series Mac, another agent and Swift builds sharing the CPU
(noisy, but the shape is clear):

| Path | Result |
| --- | --- |
| `World::task` (= `lf task create`), 64 Tasks | create 136 s, checkout 44 s (0.7 s each), 32 Sessions 2.8 s, 64 Flows 1.5 s: 3 min 7 s |
| same, 256 Tasks | 122 created after 6.5 min, per-Task cost rising ~2 s → ~4.5 s; stopped |
| store `create_task`, one planning sync at the end, no checkouts, 256 | 70 s create + 9.8 s for 128 Sessions |
| same, 2048 | 933 created after 9.5 min, ~2.7 s per Task at ~900; stopped |

Where it goes (sampled): `lf task create` runs the Git planning exchange
(`planning_git` subprocesses) per Task; below that, `planning_write::capture`
per Task write grows with the size of the plan (~16 `planning_peer_changes`
rows per Task). The lookup query in `capture` is indexed; the growing cost is
elsewhere in capture/retain and is not yet pinned down. Projected 2048 builds
are hours either way, and 2048 checkouts alone would be ~25 min.

Proposal (needs Jack's choice before building further):
1. Find and fix the per-write growth in `planning_write` (a product cost: every
   Task write in a large repository pays it), then re-measure.
2. Large recipe at a Rows rung: Tasks through store writers with one planning
   exchange, Sessions bound without checkouts; "started" needs a checkout or
   Flow, so either the perf tests stop requiring every Task started, or a few
   checkouts carry Flows and the rest stay planned.
3. Build the large world once per lf-world build and clone it per run (APFS
   clone, as `store::copy` does), so the suite pays the build rarely.

## Delete — do not maintain

- `_seed_native_task`/`_create_native_task`, the sh codex/claude stub and
  `PERF_NATIVE_ID`/`PERF_TRANSCRIPT` in `desktop_performance.py` (done).

## Remaining

- Check results: `verify-fixture` (unmounted) 5/5 passed; `world_recipes` 2/2;
  `test_desktop_performance.py` 51 passed; clippy clean; Swift test build ok.

- Mounted journeys (`run`, `verify-fixture --mounted`) not run here: they open
  windows on the host display. Gate/demo owns them.
