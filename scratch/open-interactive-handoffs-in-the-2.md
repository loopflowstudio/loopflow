# W2-177 — Open interactive handoffs in the last successful surface (serial PR 2)

PR #961 (serial 1, merged `ca81a1e3f`) delivered the CLI presentation adapter:
`lf handoff present <id>` execs into the interactive terminal and records
first-attach evidence. That closed the *CLI* gap.

This PR delivers the **remembered-surface resolution** the contract still owes —
the decision that picks *where* Open presents a handoff, and the honest set of
targets it may offer.

## What lands here

Pure, cross-platform decision model in `swift/Loopflow/Models/HandoffSurface.swift`,
plus its Proof tests. The model owns every rule the view must not re-derive:

- **Reach** — per surface, how honestly it reaches the same Session right now:
  `.attach` (runs the exact shared command), `.worktreeOnly` (opens the
  directory, never claims to attach), `.unavailable` (app/capability absent).
  - Ghostty: always `.attach` — the required, embedded local fallback.
  - Warp: `.attach` when installed and command-bearing, else `.worktreeOnly`,
    else `.unavailable`.
  - VS Code / Cursor: `.attach` only for Claude with a known provider session id
    and a proven workspace; installed-but-uncredentialed is `.worktreeOnly`.
- **Offered options** — the honest picker: every surface whose reach is not
  `.unavailable`, each labeled by the reach it delivers.
- **Memory** — last successful surface, remembered per `(provider, Home)` and
  once overall. Recorded only by an explicit `record` call the view makes *after*
  a launch succeeds.
- **Resolver** — Open's default surface, in contract order: remembered
  provider-on-Home surface → remembered overall → embedded Ghostty. A remembered
  surface is honored only while it can still `.attach`, so an uninstalled app or
  a lost capability falls back visibly to the next candidate.

## Proof (tests)

`swift/LoopflowTests/HandoffSurfaceTests.swift` covers: provider-specific
preference, overall fallback, unavailable remembered app, unsupported provider
(non-Claude IDE never attaches), remote Home keying, launch failure leaves the
preference untouched, and preference update only after success.

## Deliberately not here

The AppKit launcher glue (Warp launch-configuration write, IDE open, embedded
Ghostty wiring in `HandoffAttachSheet`) rides a follow-up serial PR — it is
side-effectful surface that consumes this model. The decision the contract turns
on is the model; that is what is proven here.
