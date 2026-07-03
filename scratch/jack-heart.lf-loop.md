# lf wave — progress arm (shipped this branch)

`lf wave <name>` (alias `lf loop`): a foreground, non-terminating command that
runs a wave's outer loop deterministically — loopflow owns the outer loop, each
pass is one bounded `lf -b goal <wave> --once`, fired the instant the last exits.
Ctrl-C or `wave/<wave>/STOP` ends it.

This branch shipped the **progress arm only**. The full four-arm runtime design
(pass launcher + monitor + cron + chat, two-tier memory, the conductor doctrine)
is folded into `wave/goals/MEMORY.md` → "lf wave runtime — the design ahead".
The remaining three arms and the Asana roadmap reconciliation owed (token was
expired this run) are tracked there too.

## Try it

```bash
# Run the goals wave's progress loop (Ctrl-C or drop wave/goals/STOP to stop):
lf wave goals

# One inner pass, standalone:
lf goal goals -b --once
```

Each pass repeats immediately on success; a failed pass waits 3s so a broken
inner run can't hot-spin. Inner passes write durable logs under the agent log
dir (the loop inherits the terminal — no separate stream capture yet).

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo test -p loopflow r#loop -- --nocapture
```

All passed locally on 2026-07-03.
