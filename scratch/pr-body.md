Background agent conversations cluttered Task navigation. Ordinary Desktop and CLI Session lists now show unfinished interactive conversations and current authored reviews, with matching navigation counts.

## What changes

- Desktop uses the shared interactive default; `--needs-me` narrows the selected mode.
- Filtered absence preserves selection, prepared commands, drafts and retained native surfaces. Explicit completion remains separate from a finished turn.
- Show headless Sessions and `lf session list --interactive all --history` retain full inspection. Task association and review authority are unchanged.

## Checks

The Desktop build and 47 focused headless tests passed after compression; an earlier broader run passed 92. API inventory and review-completion/resume integration tests passed, along with 12 Session-event tests, formatting and all-target Clippy. Gate reused unchanged-code receipts; architecture and Swift boundary checks passed. CI owns the full matrix.

Provider review coverage uses a stand-in. These checks do not establish configured provider input, native visual acceptance, sustained use or latency improvements. Direct-opening and performance work remain separate in LOO-371 and LOO-304.

## Try it

Open a Task containing interactive and background conversations: its ordinary Session list and counts should include only unfinished interactive participation. Enable Show headless Sessions to inspect background work. This is a suggested walkthrough; headless regressions cover the behavior.
