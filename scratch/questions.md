# Open questions / assumptions — W2-319

Headless run: proceeding on best judgment; these are the calls a reviewer might
reopen.

1. **Copy-based immutable global vs. symlink-into-worktree.** The design
   promotes a *copied* binary (`~/.lf/bin/lf-<rev>`, `0o555`) rather than the
   current symlink into `local-bin/`. This deliberately removes the
   "rebuild-in-worktree silently replaces the fleet" hazard that was W2-319's
   mechanism — but it also removes the convenience that a `cargo build` in a
   promoted worktree takes effect without re-running `--use`. **Assumption:** the
   isolation win outweighs the lost auto-effect; developers re-run `--use` (now
   safe and previewed). If the maintainer wants the symlink workflow kept, the
   fallback is: symlink into `local-bin/` but hard-block a bare rebuild from
   being global by having the runtime refuse to act as global if its
   `source_root` is a worktree (weaker; keeps the footgun's shape).

2. **Blanket live-body gate.** Any active/reserved lease blocks *all* global
   replacement, not only the migration-apply case. Given continuous fleet
   dogfooding this will refuse often. **Assumption:** that is intended safety and
   the refusal is actionable (names each body to drain). If it proves too
   obstructive in practice, the narrower rule (gate only frontier-advancing
   promotions on live bodies; allow same-binary/same-frontier re-promotion under
   live bodies) is a one-line relaxation — but it reopens the "swap under a
   running turn" window the incident hit.

3. **Command surface name.** Chose `lf install {preflight,promote,rollback}`.
   No existing `install`/`promote` subcommand exists, so this is a new top-level
   `Commands::Install`. If a different verb is preferred (`lf promote`,
   `lf self`), it is a rename only.

4. **Promotion lock location.** `~/.lf/promotion.lock`, distinct from the
   migration lock. Assumes the machine home is writable at promotion time (it is
   — `install.py` already writes there via skill sync).
