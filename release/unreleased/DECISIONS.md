# Release Decisions — unreleased

Append-only ledger of release-worthy intent and policy decisions for the current
cycle. Promoted to `release/v<version>/DECISIONS.md` at release time.

## 2026-06-19 — Loopflow is the layer above: hand off interactive sessions to the vendors

**Context:** We built our own interactive surfaces — mobile pairing (`lf op
pair`, QR/Tailscale), a native SwiftUI chat UI, and an lfd session-hosting layer
that parses Claude/Codex/opencode streams into our own event model (~7,800 LOC of
`lfd/sessions/harness`). Then we started tearing it back out, in place, on `main`,
twice, with nothing written down. The pattern is the tell: reimplementing the
vendors' own session UIs is work that doesn't compound. The vendors ship better
chat clients, IDE integrations, and mobile apps than we will, and they ship them
faster.

**Decision:** Loopflow is the orchestration layer. It runs headless agent work
(steps, flows, waves) and **does not host interactive sessions.** When a human
drives a session, it happens in the vendor's own surface:

- **Launch new sessions in the vendor's app** — `lf <step>` opens a fresh session
  directly in the Codex / Claude Code app, automatically, when configured that
  way. This is the headline new capability, and it is terminal-first: a plain
  `lf` invocation does it; Concerto is just another caller.
- **Embedded terminal** stays — a tmux pane running the vendor's own TUI (Claude
  Code, Codex CLI, opencode) inside Concerto. Concerto frames it; the vendor
  renders it.
- **Bounce to the vendor's IDE** — open the worktree in VS Code / Cursor /
  JetBrains where the vendor's extension runs the session. Not websites — IDE
  integrations.
- **opencode → TUI only**, for now.

Concerto stays as the macOS surface, raised a layer: wave monitoring plus the
frame around vendor TUIs. It is no longer a chat client.

**Implications:**

- **Dropped:** native SwiftUI chat UI; the `lfd/sessions/harness` stream-parsing
  layer (it existed only to feed the native UI; confirmed separable from the
  embedded terminal); the entire mobile surface — iOS target, `lf op pair`,
  pairing tokens, remote-lfd-for-phone connection infra.
- **Kept:** the embedded terminal (tmux-backed pane).
- **New build:** config-driven vendor-session launch from `lf` and Concerto. The
  open question is the launch mechanism each vendor exposes (URL scheme vs CLI vs
  `open -a`) and how much context (worktree, initial prompt) it accepts — a spike
  gates the config design.
- **Mobile wave archived.** Mobile happens through the vendors' own mobile apps,
  not a loopflow iOS app.
- The in-place teardown on `main` was dropped (stashed) in favor of doing it
  deliberately as part of executing this thesis.
