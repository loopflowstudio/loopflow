# Concerto lfd-free basic UX — how to evaluate

The slice shipped: Concerto lists disk-authored waves, launches a wave's `/goal`
loop in tmux via `lf goal <wave> --tmux`, and attaches — with lfd out of the
launch/attach path. Implementation is in the code (`goal.rs` `launch_in_tmux`,
`WavesView.swift`, `PortfolioRepoState.startWaveAgent`); this doc is only the
reviewer's how-to-exercise.

## Try it

```bash
cargo build --bin lf
target/debug/lf goal concerto --tmux          # creates ../loopflow.concerto, launches loop
tmux ls                                        # shows lf-loopflow-concerto in ../loopflow.concerto
tmux attach-session -t lf-loopflow-concerto
target/debug/lf goal concerto --tmux          # idempotent: reprints the same handle, no dup
```

Then launch Concerto against this repo and **kill lfd**: disk-authored waves
should still render (status from `tmux has-session`), and selecting one should
launch + attach through `lf goal <wave> --tmux`.

## Done when (the checks)

1. `lf goal <wave> --tmux`, run from any checkout:
   - ensures `../<repo>.<wave>` exists (creates it if missing),
   - starts the goal loop in a detached tmux session whose cwd is that worktree,
   - prints the session name and exits 0,
   - idempotent: a second call with the session live just reprints the handle.
2. In Concerto, clicking a wave launches + attaches its goal terminal, and the
   wave list renders from disk — no lfd query in that path.
3. `tmux ls` shows `lf-<repo>-<wave>` running in `../<repo>.<wave>`.

## Failure modes to exercise (tests, not the demo)

- **lfd absent:** kill lfd — `lf goal --tmux` still launches + prints a handle,
  and Concerto's list + click-launch still work.
- **Idempotent relaunch:** second `--tmux` with the session live reprints the
  handle; no duplicate session.
- **Create-if-missing:** fresh repo with no `../<repo>.<wave>` → worktree created,
  loop runs there.
- **Stale handle:** `lf-<repo>-<wave>` recorded but the tmux session died →
  relaunch recreates cleanly.

The forward-looking reduce roadmap this slice seeded now lives in `wave/reduce/`.
The remaining Concerto gaps (launch beyond `/goal`, registry-backed live status,
goal-resolution in the launched worktree) live in
`wave/concerto/1-embedded-terminal-build-driver.md`.
