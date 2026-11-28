# Settled direction & open notes

- **No parity table or `Support::Gap` (Jack Heart, 2026-11-24):**
  - `"i would rather make it harder to add a harnes if it makes it harder to miss imortant behavior. when i add a harness, i want it to have all the good stuff"`
  - `"1. Just remove the table. Instead, just make sure that all behaviors are asserted to work and testeed  across all harnesses."`
  - `"and that the Harness interface demands full functionality"`
  Remove `harness/parity.rs` and the `harness/README.md` parity table; put required mechanisms directly on `Harness` / `Launcher` / `Capabilities` and assert every behavior unconditionally across `HarnessKind::ALL`, isolating only pre-existing gaps (now just Task 48edb9 headless questions) as narrow inline exceptions with no wildcards.
- **Reopen on Antigravity:** Use Antigravity's native interactive prompt (`-i "Read the waiting steers and Task updates for this Session."`) to fire `PreInvocation`, keeping steers unread until the hook prints them. Open UX detail noted in QA: that generic opening line is visible as the first turn in the TUI.
- **Already-open idle Antigravity wake:** back in LOO-1098 (see the ship-review section below); `?2004h` alone is not readiness, so `prompt_ready` reads the whole screen.
- **Viewer handoff:** Owned by LOO-1097 (live provider TUI behavior).

- `AI_AGENT` stays in the shared markers. A nested launch clears it regardless of which provider set it.

# Probe side effect (2026-11-24)

While probing, two `agentapi send-message` calls from this checkout's shell
used `ANTIGRAVITY_*` values inherited from conversation
`c3742012-2e46-47c2-a718-aa39e90fe213` (agy pid 49362, the one that ran
`lf task run LOO-1098`). Both messages went to throwaway probe conversations
in `/tmp/agyprobe`, but they were sent through 49362's server and attributed
to `c3742012`. That conversation may show those two sends.

# QA pass 2 (2026-11-25)

- Same steer, different outcome: on the `task-session` primary, Codex (steer as
  its own turn) answered the steer; Claude (cross-session message) reported it
  and waited "when you ask here", on both idle wake and reopen. Delivery is the
  same; behavior differs. Likely skill/primary-session framing, outside this
  diff. See scratch/qa.html.

# Pass 3 steer — display steers inline in Antigravity (2026-11-25)

- Jack Heart (`comment:79dd230d-1028-4f14-acb2-90b07b1f3804`):
  - `"is this also ging to make it so that agy *displays* the steers it gets"`
  - `"Just go ahead and put this in the task please, lets go back"`
- Proposed formatting before the native check: render clean markdown in
  `metadata.user_markdown` (`**Steer** (<author>)`, `**Task update**`,
  `**Task notice**` + raw body, truncated cleanly only above 2 KB, plus quiet
  `N more waiting` footer) rather than echoing raw `<lf:steers>` XML / prompt
  log paths inline.

- Native QA ruled out `metadata.user_markdown` on 1.5.16 and 1.5.17.
  Kept plain system-message delivery. See the remaining decision in the plan.

# Ship-review direction — fold live idle agy wake back into LOO-1098 (2026-11-25)

- Jack Heart:
  - `"b"` (for inline display → filed as LOO-1103)
  - `"wait this was deferred? isnt this the main thing i asked for? Already-open idle agy wakeLive idle agy work sessions do not wake until a typed turn or reopen; blocked on a safe provider route."`
  - `"Test the sidecar route now ( and then Revisit held-terminal wake if necessary"`
  - `"yes A"`
  - `"do it in 1098 and abandon 1099"`
- Option A: VT-guarded held-terminal wake pasting the short trigger prompt (`"Read the waiting steers and Task updates for this Session."`) + `\r` so `PreInvocation` claims and stamps steers only after `agy` invokes the hook. LOO-1099 abandoned.

# Ship-review answers (2026-11-27)

- Jack Heart: `"1b 2a 3a 4a"` (drop Claude cross-session steer framing Task, drop `lf session steers | head` broken-pipe Task, keep 2s quiet pause, sync with `main` and deliver PR #2045).
