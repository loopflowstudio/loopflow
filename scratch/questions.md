# Open choices (made headless, for review)

- Antigravity global skills: generated tree at `~/.gemini/config/skills/`, not a
  `skills.json` entry. Cost: third-party bundles in `~/.agents/skills`
  (`find-skills`) don't show in agy. Reasons and probe results in `plan.md`.
- The tree is written even when Antigravity isn't installed, matching
  `~/.claude/skills` and `~/.agents/skills`. Alternative: write a harness's
  global directory only when its home (`~/.gemini`) exists.
- One rendering for every directory: Graph exports carry
  `disable-model-invocation: true` in `.agents/skills` too, so pi stops picking
  Graph recipes on its own. Passed over: a dialect per directory. Codex 0.170.0
  still lists Graph skills carrying the key; whether it honors it is unchecked.
- A harness that moves its skill directory leaves the old generated tree behind;
  removed by hand.
- No "this harness has no installed-skill location, because…" variant; the
  conformance test rejects an empty declaration instead.
- `native_skills` stays `None` for antigravity, opencode and pi. Possible later
  Task: invoke by name (`/name` in agy print mode, `/skill:name` in pi).
- agy 1.5.14 ignores `~/`-relative `entries` in the global `skills.json` although
  its bundled docs say they resolve; absolute paths work. Not reported upstream.
- The brief calls the command `lf skill sync`; the command is `lf self sync-skills`.
- The shared repository (`/Users/jack/src/loopflow/.git/shallow`, written
  2026-11-21 08:47, before this run) is marked shallow at origin/main's tip.
  `loopflow-storage/build.rs` then gets no migration order and every build in
  every worktree fails at "apply graph_process_task: no such column: is_local".
  History is still on disk. This run left the repository alone and built with
  `LOOPFLOW_MIGRATION_ORDER` set to an order read with `GIT_SHALLOW_FILE`
  pointing at an empty file. `git fetch --unshallow origin` would repair it for
  everyone; not run here because the cause is unknown.
