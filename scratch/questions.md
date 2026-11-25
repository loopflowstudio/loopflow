# Open choices (made for review)

- **Wake `agy` by typing sanitized bracketed paste into its held terminal.**
  The alternative is a provider inbox, and `agy` 1.5.16 exposes none an outside
  process can reach (see `scratch/plan.md`). The steer appears in the TUI as a
  pasted prompt, not as a `[Message]` from a sender. If `agy` later loads
  workspace sidecars or gives hooks `ANTIGRAVITY_CSRF_TOKEN`, an inbox through
  `agentapi send-message` replaces `IdleWake::Terminal` with no wire or schema
  change.
- **Draft guard tracks actual user keystrokes (ignoring focus, terminal replies,
  and mouse events), unsubmitted input, and an in-server `yielded_at` `Instant`**
  rather than reading the prompt line off the VT screen or comparing `SystemTime`
  against SQLite's whole-second `observed_at`. A person who types characters and
  manually backspaces them all out without pressing Enter, Ctrl-C, or Ctrl-U
  waits until their next Enter (which delivers steers via `PreInvocation`).
- **Move all idle-wake reach dispatch out of SQL into `Capabilities.idle_wake`**
  and return plain columns (`provider`, `endpoint`, `inbox`, `socket`,
  `held_terminal_id`) from `idle_conversation_candidates`, with flat
  `IdleReach::Terminal(TerminalId)`. Keeps SQL harness-agnostic and avoids
  guessing terminals by session ID.
- **Separate `Verification::Automated` from `Verification::RealProvider` in
  `harness::parity`.** The 12 behaviors stand-ins can exercise run over
  `HarnessKind::ALL` and assert `Support::Yes` and `Support::Gap` both ways.
  `ViewerHandoff` (Antigravity `Gap`, owner LOO-1097) is a real-provider TUI
  issue that stand-ins cannot honestly reproduce, so it is tracked as
  `RealProvider` instead of mocking a fake failure. `UnansweredQuestion`
  (Claude, Codex, Antigravity `Gap`, owner Task 48edb9) stays tracked and
  checked both ways across `HarnessKind::ALL`.
- **Reopen `agy --conversation <id> -i <prompt>` verified live in Pass 2.**
  If `agy` ignores `-i` when `--conversation` is passed, `take_steers_to_start`
  leaves waiting steers unread for `IdleWake::Terminal` so terminal wake
  delivers them once on `?2004h` instead of marking them delivered and dropping
  them.

# Probe side effect (2026-11-24)

While probing, two `agentapi send-message` calls from this checkout's shell
used `ANTIGRAVITY_*` values inherited from conversation
`c3742012-2e46-47c2-a718-aa39e90fe213` (agy pid 49362, the one that ran
`lf task run LOO-1098`). Both messages went to throwaway probe conversations
in `/tmp/agyprobe`, but they were sent through 49362's server and attributed
to `c3742012`. That conversation may show those two sends.
