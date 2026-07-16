# W2-177 — Honest handoff surfaces + launcher (serial PR 3)

PR #969 (serial 2, merged `cadceca23`) landed the surface-resolution *model*, but
it classified VS Code and Cursor as `.attach` while no launcher could resume the
specific durable Session. Review flagged the mismatch: a folder-open does not
attach a Session, so the label lied.

This PR makes the user-visible capability match the actual launch behavior, and
adds the launcher that consumes the model.

## The honesty fix

Only a surface that runs the **exact shared attach command** the store hands back
may claim `.attach`:

- **Ghostty** — embeds and runs `attach.argv`. Always the required fallback.
- **Warp** — writes a command-bearing launch configuration that runs `attach.argv`
  in the worktree, then opens `warp://launch/<name>`. If the config can't be
  written, the launch *fails* (visible fallback to Ghostty) rather than opening a
  bare window and calling it "attached".
- **VS Code / Cursor** — no launch resumes the specific provider Session, so they
  are **`.worktreeOnly`**: offered (when installed with a proven workspace) as
  "… (open worktree)", never claiming to attach. Removed the dishonest
  `providerIsClaude`/`providerSessionKnown → .attach` path and the now-unused
  capability fields.

The picker labels each option by the reach it delivers; a worktree-only option
never overclaims. The resolver honors a remembered surface only while it can
still `.attach`, so a remembered IDE always falls back to Ghostty.

## Launcher + preference wiring

- `HandoffSurfaceLauncher` — installed-app probe (`NSWorkspace`), capability
  construction, and the per-surface launch. Pure `warpLaunchConfigYAML` renders
  the command-bearing config (testable, no filesystem).
- `HandoffSurfacePreferences` — persists `HandoffSurfaceMemory` to `UserDefaults`,
  recording only after a launch succeeds.
- `HandoffAttachSheet` — resolves the default surface, offers the honest menu,
  embeds Ghostty or launches externally, records the preference only on a
  *user-initiated* success (an auto-resolved fallback never rewrites memory), and
  falls back visibly on launch failure.

## Proof

Launch-level tests (`HandoffSurfaceLauncherTests`):
- the Warp attach config embeds the **exact provider-session-bearing argv**
  (`'claude' '--resume' 'sess_…'`), so Warp attaches the same Session, not a shell;
- every argv token survives quoting; a quote can't break out of the command;
- an IDE bears **no** session action — reach is `.worktreeOnly`, and no offered IDE
  option ever advertises attach.

Model tests (`HandoffSurfaceTests`) updated for the honest reach.
`swift test --filter HandoffSurface` → 14/14 pass. `swift build --target LoopflowMac` clean.

### Manual same-session trial

Staged through the store (throwaway `wave:` parent, cleaned up to terminal):

```
lf handoff open --provider claude --provider-session sess_… -- claude --resume sess_…
lf handoff attach <id> --json   # attach #1
lf handoff attach <id> --json   # attach #2 (reopen)
```

Both attaches returned the **same** `session_id` and the **same** `argv`
(`['claude','--resume','sess_w2177proof']`) — reopening reaches the same durable
Session, and the command carries the provider session id. Ghostty and Warp both
run this identical argv, so every attach surface reaches that one Session. The IDE
surfaces make no such claim.

Full in-app UI trial (embedded Ghostty + external Warp/IDE windows on a real
handoff) needs a display and is left to an attended run; the machine-checkable
descriptor stability above is what a headless run can prove.
