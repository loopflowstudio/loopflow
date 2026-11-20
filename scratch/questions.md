# Open choices (4bd948)

Made headless during kickoff; each is reversible in the diff.

- **Task-checkout yolo rule deleted.** Under `yolo: false`, a headless Codex
  Task step now runs in `workspace-write` and cannot reach the network or
  `~/.lf`. The alternative keeps the rule and has headless Task steps ignore a
  written `yolo: false`.
- **Headless floors kept unchanged.** `yolo: false` therefore changes nothing
  for headless Claude, OpenCode or Antigravity; only Codex gets a sandbox.
- **`--yolo` kept, no `--no-yolo` added.** Off is `--setting yolo=false`.
- **Not probed:** Claude's refusal of `--dangerously-skip-permissions` as
  root, which attended launches on a root Machine would now meet.
