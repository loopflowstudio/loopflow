# Open questions — W2-240 reject unknown explicit wave names

All four resolved during kickoff de-risking (verified against source on this
branch). None block implementation.

1. **Does `lf home start --wave <name>` create a wave, or require one to
   exist?** RESOLVED. `start_home` (`ops/home.rs:156`) launches `lf wave
   <name>` in a detached tmux session, which registers the row on first run —
   so the *positional* `lf home start <name>` is a creation path and bypasses
   validation. The `--wave` *flag* form does NOT reach `start_home` as a
   creation path: it hoists to the global `resolve_explicit_wave`
   (`reorder_args` is clap-derived; `HomeCommand`'s `wave` is positional only),
   which already rejects unregistered names today. So only the positional form
   needs `allow_unregistered_explicit: true`; the flag form is a message-only
   change (`not found` → `UnknownExplicit`).

2. **Does `lf pm init --wave <name>` require the wave to already be
   registered?** RESOLVED. `pm_init_async` (`ops/pm.rs:947`) requires the wave
   *directory* (`wave/<name>/`), not a registry row, and resolves the name with
   normalize-only (`ops::util::resolve_wave_name`). Its `--wave` flag
   (`wave_flag`, `mod.rs:1247`) stays pm-local under `reorder_args`, so the
   global validating gate never sees it. Creation-safe; bypasses validation.

3. **Opt-out shape: boolean parameter vs. a second function.** RESOLVED.
   `allow_unregistered_explicit: bool` on the shared resolver. One rule, the
   exception visible at each call site. Call-site count is small (creation
   flows: `lf wave`/`stop`/`resident` positional, `lf home start` positional,
   `lf pm init`) — no second function needed.

4. **`resolve_target`'s ambient `row=None` path.** RESOLVED. Kept as
   `Ok(None)` (the "no subscriber, broadcast drops" case for default targeting
   with no wave context). Only the *explicit* `args.wave` arm errors with
   `UnknownExplicit`. No chat/memory flow relies on an explicit unknown name
   resolving to `Ok(None)` — that silent accept is exactly the bug being fixed
   (memory `show --wave definitely-unknown` exits 0 with empty output today).
