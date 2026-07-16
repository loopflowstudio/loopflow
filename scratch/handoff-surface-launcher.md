# W2-177 — Honest handoff surfaces + launcher (serial PR 3, #978)

PR #969 (serial 2) landed the surface-resolution model but classified VS Code and
Cursor as `.attach` while no launcher could resume the specific durable Session.
This PR makes the user-visible capability match the actual launch, adds the
launcher, and — after a second review — fixes the contract holes below.

## The honesty fix

Only a surface that runs the **exact shared attach command** may claim `.attach`:

- **Ghostty** — embeds and runs `attach.argv`. The required fallback.
- **Warp** — writes a command-bearing launch configuration that runs `attach.argv`
  **with the descriptor environment preserved** (an `env KEY=VALUE …` prefix, since
  Warp configs carry no environment field), then opens `warp://launch/<name>`. A
  config-write failure *fails* the launch (visible Ghostty fallback) rather than
  opening a bare window and calling it "attached".
- **VS Code / Cursor** — no launch resumes the specific provider Session, so they
  are **worktree-only**: offered (installed + proven *local* workspace) as
  "… (open worktree)", never claiming attach. This is the truthful *unsupported-for-
  attach* classification; the surfaces are still offered, so the task is not weakened.

## Review fixes (this round)

1. **Warp preserves the descriptor environment** — `warpLaunchConfigYAML` now takes
   the descriptor `environment` and renders it as a sorted `env …` prefix on the
   exact argv. Proven by `warpConfigPreservesEnvironment`.
2. **Worktree-only never overwrites the last valid attach preference** — recording
   is gated by the pure rule `handoffPreferenceShouldRecord(reach:userInitiated:
   launchSucceeded:)`, which records **only** a user-initiated, successful, `.attach`
   launch. A worktree-only IDE open, even user-initiated, leaves the remembered Warp
   attach intact. Proven by `worktreeOnlyDoesNotOverwriteAttachPreference`.
3. **Fallback reasons are shown** — `HandoffSurfaceResolver.resolve` returns a
   `HandoffSurfaceResolution { surface, fallbackReason }`. An unavailable remembered
   app → "Warp is unavailable — using the embedded terminal."; capability loss →
   "Warp can no longer attach — …". The sheet shows it in the header. Proven by
   `unavailableRememberedApp` / `capabilityLossFallsBack`.
4. **Attached-but-unresolved handoffs stay red** — the census keyed redness on
   `status == .waiting`, so an `.attached` handoff went green and stopped reddening
   its parents. Since the census only holds *active* handoffs (waiting **or**
   attached), all of them now redden Task → Project → Wave and name the human until
   they reach a terminal outcome. Proven by `attachedHandoffStaysRed`.
5. **Real launch-path tests** — the unreachable `if launchSucceeded {}` branch is
   gone; recording, the Warp config renderer, env preservation, quote-escaping, and
   remote-Home detection are tested directly.
6. **Remote Home consumes `descriptor.host`** — `capability(host:cwd:)` computes
   `isRemoteHome`; on a remote Home the worktree is not local, so IDEs and plain
   Warp windows are `.unavailable`, while Ghostty and command-bearing Warp still
   `.attach` (the shared argv carries its own ssh transport). The local workspace is
   never probed for a remote Home. Proven by `remoteHomeShapesReach`,
   `remoteHomeDetection`, `remoteHomeIsNeverLocallyProven`.

Durable descriptor/store still wins: the view creates and names nothing; every
surface runs the argv the store returns.

## Proof

`swift test --filter "HandoffSurface|ActiveSessions"` → 28/28. `swift build
--target LoopflowMac` clean.

### Manual same-durable-session evidence

Staged through the store (throwaway `wave:` parent, cleaned up to terminal):

```
lf handoff open --provider claude --provider-session sess_… -- claude --resume sess_…
lf handoff attach <id> --json   # attach #1
lf handoff attach <id> --json   # attach #2 (reopen)
```

Both attaches returned the **same** `session_id` and **same** `argv`
(`['claude','--resume','sess_w2177proof']`) — reopening reaches the same durable
Session, and the command carries the provider session id.

Per-surface:
- **Ghostty** runs that exact `argv` (embedded), so it reaches that one Session — the
  descriptor-stability trial above is its evidence.
- **Warp** renders the same `argv` + environment into its launch config; the exact
  rendered command is pinned by `warpConfigCarriesTheSessionCommand` /
  `warpConfigPreservesEnvironment`, e.g.
  `exec: "'env' 'LF_WAVE_ID=w_42' 'TERM=xterm-256color' 'claude' '--resume' 'sess_abc123' …"`.
- **VS Code / Cursor** open the worktree only (`NSWorkspace.open([folder], withApplicationAt:)`)
  and never claim attach — proven by `ideBearsNoSessionAction`.

The full in-app UI trial (real handoff rendered in embedded Ghostty and external
Warp/IDE windows) needs a display and is left to an attended run; the descriptor
stability and the exact launch commands above are what a headless run can prove.
