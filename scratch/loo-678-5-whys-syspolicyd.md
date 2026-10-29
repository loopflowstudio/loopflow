# LOO-678: Sessions end when their lf build is gone

Status: draft design (kickoff, 2026-10-28; reconfirmed in kickoff the same day). Jack Heart asked for a 5-whys investigation,
then a fix and shipped PR (Task comment, 2026-10-28).

## Problem

On 2026-10-28 (~15:00–16:15 PDT) syspolicyd sat at 600–800% CPU on Jack's Mac. Every
rejected path was an lf image that a long-running Session owner had started from and
that no longer existed in that form: a deleted worktree's `target/debug/lf` (ce4160, a
26-hour-old Codex Session, the bulk of the storm), `Loopflow Dev.app/Contents/MacOS/lf`
after a 14:07 rebuild (two LOO-627 demo Sessions in `/private/tmp/lf627`, one Oct 27
Codex Session), three Oct 27 worktree builds (628, 643, 604 demo/QA Sessions) and a
deleted test stand-in `claude`. Stopping exactly those process trees by verified PID
fixed it. Pausing Rust tests did not.

Beneficiary: anyone developing Loopflow on their own Mac, who rebuilds lf and removes
worktrees all day and should never have to find and kill leftover Sessions.

## Demo

```sh
cp target/debug/lf /tmp/lf-demo && /tmp/lf-demo -i -a codex : "say ready"   # detach with Ctrl+]
rm /tmp/lf-demo
lf session list            # within ~2 s the Session shows ended, not detached
tail -1 ~/.lf/logs/lf.log  # "ending a Session whose lf executable was removed" path=/tmp/lf-demo
lf resume <session>        # reopens the conversation under the installed lf
```

Live acceptance (Jack, after merge): rebuild Dev.app and remove a worktree while Sessions
from each are open; `top -l 2 -stats pid,command,cpu | grep syspolicyd` stays near 0%.

## Five whys

1. **Why did syspolicyd spin?** The kernel kept asking it to assess launches attributed to
   lf images it could no longer read (`errno 2`), and each was rejected as malware. 388
   rejections in 10 s on consecutive PIDs means something kept launching.
2. **Why were lf processes running from images that were gone?** Session owners
   (`lf session serve` → `lf -i`/`lf resume` → provider) run from whatever binary started
   them, indefinitely. Worktree removal, `target/` build cleanup, `cargo build` and Xcode
   rebuilds replace or delete that binary underneath them; nothing in lf notices.
3. **Why didn't the detached-Session limit end them?** It ends an owner only 24 h after its
   last viewer leaves and only while it waits for a person. The demo Sessions were ~2 h old;
   the ce4160 one survived 26 h (its viewer/turn state is unrecoverable: it ran on a
   dev-build store copy whose WAL was never captured). The transport README also promises a
   Machine-wide sweep ("every owner judges all owners… owners from builds that never
   recorded that time end 48 hours after they started") that no code implements.
4. **Why do children follow the vanished build?** By design: `transport::start` launches
   `session serve` and argv[0] from `current_exe()`; `resolve_lf_binary` returns
   `current_exe()` in store-copy mode, `LF_BIN` under a private Home, and the Dev.app helper
   path from Desktop. Hooks embed that absolute path (`context_block::hook_settings`), and
   Codex receives it as `LF_BIN`. That is correct while the build exists.
5. **Why does nobody own stopping demo/QA Sessions?** The demo skill "closes no Session",
   proof scripts (e.g. `/tmp/ce4160-proof/proof.py`) start Sessions from worktree builds and
   exit, and Task completion removes the checkout. Ownership of the Session ends with the
   person's attention; only the owner process itself is present for its whole life.

Root cause: an owner's lifetime is tied to viewers and turns, never to the executable it
runs. The fix ties it to the executable: the one participant always present decides.

### Findings that shape the design

- **Exact spawner: unresolved.** Kernel/syspolicyd lines are gone from the unified log, the
  storm children died before ledger admission (no rows in `~/.lf/loopflow.db` or
  `/private/tmp/lf627/home/loopflow.db` after 19:54 UTC), and ce4160's ledger was a store
  copy. The serve loop, idle watcher, lifeline and Codex relay start nothing periodically;
  `task run` relaunches use 1–30 min backoff (`ops/task/ship.rs:107`).
- **Kernel trigger: not reproduced by small probes.** Fork, fork+exec of a platform binary,
  posix_spawn of a linker-signed non-platform child, and spawning from a deleted
  *responsible* process (disclaimed launch) all succeeded from deleted and replaced
  images (2026-10-28, three probe classes, no load). The kill needs more than a vanished
  image; the design therefore removes the precondition instead of fencing one trigger.
- `lf install` already keeps artifacts that live processes run (`prune_superseded_artifacts`
  + `running_executables`), so installed `~/.lf/bin/lf-<hash>` owners are not exposed. The
  gap is binaries lf does not own: worktree builds, Dev.app, test fixtures.
- **Installed owners must survive `lf install`.** `~/.lf/bin/lf` is a symlink swapped by
  atomic rename (`commit_cli_symlink`), so the rule watches `canonicalize(current_exe())`,
  i.e. `lf-<digest>`, never the symlink. A same-digest reinstall keeps the existing file
  (`stage_binary_as` only re-chmods it: ctime changes, inode/len/mtime do not), so identity
  is `(dev, ino, len, mtime, mtime_nsec)` without ctime.
- `owner_waits_for_person` ignores viewers (open Session, interactive, no Graph step, no
  running turn), so "viewers notwithstanding" needs no new query. While a gone owner is
  busy it re-runs that query every 2 s until the turn settles; bounded and rare.
- `platform::kill_process` ends an owner by spawning `/bin/kill`: ending itself is one more
  launch from the vanished image.

## Chosen approach

**Every Session owner watches its own executable and ends when it is removed or replaced.**

- At `transport::serve` start, record the identity of `current_exe()`: path plus
  `(dev, ino, len, mtime)` from `fs::metadata`.
- The owner's existing watcher thread (`transport/idle.rs`, renamed to what it now is:
  the owner's lifetime policy) wakes at least every 2 s and stats the path. Missing file or
  changed identity = **executable gone**. One `stat` per owner per 2 s.
- Executable gone and the owner waits for a person (`owner_waits_for_person`) → end now,
  **viewers notwithstanding**. Holding a turn or a Graph step → end when it settles,
  checked each tick. Headless Runs finish on their own, as today.
- End through the existing path (SIGTERM to self → agent stops, exit recorded) so
  `lf resume <session>` reopens from provider history under whatever lf the person runs,
  and `lf resume --all` leaves it alone.
- `platform::kill_process` signals with `libc::kill` instead of spawning `kill` (all four
  callers). Ending must launch nothing.
- One `tracing::warn!` with owner, Session, path and `removed|replaced` → `logs/lf.log`, so
  the next occurrence names itself.
- Correct the transport README: document the executable rule; delete the Machine-wide sweep
  and 48-hour claims.

This covers every observed source at once — worktree removal by any means (`lf wt rm`,
build cleanup, `rm -rf`), Dev.app rebuild, cargo relink, deleted fixtures — without
teaching each remover about processes.

## Alternatives rejected

- **Worktree removal stops owners whose image lives in the checkout first.** Covers one of
  four sources, adds a process inventory to every remover, misses Dev.app/cargo/`rm -rf`.
  A second mechanism for the same failure.
- **Re-resolve children to the installed lf when the build vanishes.** The owner keeps
  running a vanished image and its provider/hook config already embeds the old path;
  copy-mode and private-Home Sessions would silently switch stores/builds.
- **Refuse to spawn when `current_exe` is gone.** A guard at N call sites, and the trigger
  isn't proven to be lf's own spawns (hooks run via the provider).
- **Shorter `end_detached_after`.** The demo Sessions were 2 h old; any limit short enough
  ends real conversations.

## Contract

- **User-visible outcome:** a Session whose lf binary is deleted or rebuilt ends within
  ~2 s of being idle (or when its current turn settles); it shows ended in
  `lf session list`/Desktop and resumes with `lf resume`.
- **Source of truth:** the owner's `processes` row (completion) and Session history, written
  by the existing exit path. No new persisted field.
- **Affected surfaces:** `transport.rs` (identity at serve start), `transport/idle.rs`
  (policy), `platform/mod.rs` (`kill_process`), `transport/README.md`. No DTO, schema or
  Swift change; Desktop reads the existing ended state.
- **Absent/error states:** `current_exe()` or the first `metadata` fails → no executable
  rule for that owner, one warning; detached limit still applies. A transient stat error
  other than NotFound is not "gone" (retry next tick). `end_detached_after: off` does not
  disable the executable rule.
- **Operational boundary:** one stat per owner per 2 s; no subprocess, no network.
- **Exclusions:** auto-resuming rebuild-ended Sessions under the new build; test fixture
  process leaks (stand-ins outliving a test) — the rule ends leaked `lf` owners on the
  next relink, the stand-in itself is the test's; the demo skill text; Linux (same code,
  `/proc/self/exe` semantics differ but `current_exe()` path + inode compare holds).

## Delete — do not maintain

- `platform::kill_process`'s `Command::new("kill")` body (replaced by `libc::kill`).
- README claims of a Machine-wide owner sweep and the 48-hour rule for old builds.

## Forbidden outcomes

- A per-remover process check (wt rm / build cleanup / Task completion) alongside the rule.
- A second watcher thread; the owner policy is one loop.
- Ending a Session mid-turn because of a rebuild.

## Slices

One PR.

1. **This slice (implemented 2026-10-28):** identity + policy + `libc::kill` + README
   (`transport/lifetime.rs` replaces `idle.rs`) and the focused test
   `tests/transport_tests.rs::sessions_end_when_their_lf_executable_goes`: viewed owners
   served from copies of `CARGO_BIN_EXE_lf` in the fixture Home; removed and replaced
   copies end within 10 s, a busy owner waits for its activity row to clear, and an
   owner launched through a swapped symlink keeps running. `Fixture::command_from` runs
   lf from another executable; the waiting `claude` stand-in, launch log, owner
   running/ends/stop and working-turn helpers are `Fixture` methods shared with the
   detached-limit and resume-all tests. Remaining: gate, publication, Jack's live acceptance.

## Done when

- Deleting or replacing the binary under an idle, viewed Session ends it within 10 s; a
  busy one ends after its turn; both resume. (`cargo test -p loopflow --test transport_tests
  sessions_end_when_their_lf_executable_goes < /dev/null`, passes headless.)
- `detached_sessions_end_after_their_limit` still passes.
- The focused test serves one owner through a symlink to the copy and swaps the symlink to
  another file; that owner keeps running (installed Sessions survive `lf install`).
- `grep -n 'Command::new("kill")' rust/loopflow/src/platform/mod.rs` → no match.
- Jack's live acceptance above (post-merge verify).

Check results: `cargo test -p loopflow --test transport_tests -- sessions_end_when_their_lf_executable_goes detached_sessions_end_after_their_limit resume_all_restores_interrupted_sessions_and_leaves_left_ones` pass, `cargo clippy -p loopflow --all-targets -- -D warnings` clean after compress (2026-10-28). Gate owns affected suites.
