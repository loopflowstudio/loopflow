# 01: tmux Architecture Study

**Finish line:** understand which parts of tmux's server/client architecture should directly inform loopflow's later daemon-hosted shell model, and which parts should stay tmux-specific now that local v0 is shared-store observation plus ordinary local terminals. Keep the long-term target in view: loopflow should become the best way to host SSH-style sessions into coding agents, not just a local terminal embedding trick.

## Why study tmux

tmux already solved the hard parts of long-lived terminal hosting:
- server/client split
- detached and reattached sessions
- stable identity for sessions, windows, and panes
- PTY ownership living on the server
- multiple clients attached to one live session

`lfd` should not clone tmux's product surface, but it should learn from the shape of the system.

The key constraint for this study: PTY hosting is not step zero anymore. First establish one CLI-native execution model and one shared runtime store. Then borrow the right tmux ideas when remote access, reconnect, and multi-client attachment become the real pressure. The local-first staircase is there to keep the architecture honest while we aim at a better remote shell host later.

## Questions to answer

1. **Server boundary.** Which tmux server responsibilities still matter if `lf` already owns execution semantics and writes structured lifecycle data to a shared store before `lfd` owns PTYs?

2. **Identity model.** Which tmux concepts map cleanly onto loopflow later, and which should stay out of the first local model?
   - tmux session
   - tmux window
   - tmux pane
   - client attachment

3. **Detach/reattach semantics.** What state survives disconnect? What is live-only? What has stable IDs? Which of these matter only once we move beyond local Ghostty sessions?

4. **Control protocol.** What can we learn from tmux control mode and socket protocol about keeping transport structured instead of scraping terminal text once remote shells actually arrive?

5. **Multiplexing policy.** Where should loopflow borrow tmux's plurality — multiple attached clients, multiple live shells — and where should Concerto intentionally stay calmer and foreground one run? In particular, how should one mobile + desktop pair attach to the same live session?

6. **History model.** What should be durable in v1? The current answer is likely "structured history in the store, not full terminal scrollback." What tmux lessons still matter under that constraint?

7. **Remote shape.** Which tmux ideas make remote shells safer and simpler if remote starts as SSH into a host/container before loopflow invents a custom PTY transport? Which of those ideas are necessary if the end goal is to be the best host for SSH-style coding-agent sessions rather than a generic terminal multiplexer?

## What to produce

- A short architecture note under `scratch/` or `wave/lfd/` comparing:
  - tmux server/client/session/window/pane
  - shared runtime store / `lfd` / run / terminal session / worktree / client attachment
- A clear list of concepts to copy, adapt, and avoid
- A recommendation for the staircase:
  - v0 shared-store observation
  - local Ghostty embedding
  - later PTY hosting / remote transport

## Done when

- We can explain the later `lfd` shell model in tmux-like terms without confusing wave/run identity
- We know whether multiple attached clients, scrollback persistence, and pane-like composition belong in the first PTY transport or later
- We know which tmux ideas still matter even if local v0 has no daemon-owned PTY at all
