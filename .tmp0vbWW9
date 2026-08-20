# v0.12.9

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.9 makes Task work easier to observe and safer to hand to a person while it is still in motion. Initialization is now durable before a worktree exists, routine control reads stay focused on the current repository, and human gates preserve both context and progress. The result is a quieter terminal control surface with fewer ambiguous states at the moments when work changes hands.

## See Tasks while their worktrees are being placed

A newly published Task no longer disappears into the gap between durable creation and a usable worktree. Loopflow records initialization with the Task, PR, Steer, and reserved Run, then closes that state only after placement completes, so operators can distinguish active setup from stale or missing Git state (#1199).

- `lf task status <task> --json`, `lf task wait <task> --timeout 0s --json`, and `lf roadmap --wave <wave> --json` keep the Task visible during placement.
- Status, wait, roadmap, chat, completion, and supervision recognize initialization without trying to inspect an invalid worktree.
- Stale and genuinely missing worktrees remain distinct from active initialization, preserving the existing recovery paths.

## Keep shared control views local to the repository

Normal terminal control now shows the work owned by the repository from which `lf` is invoked. This keeps unrelated Waves and User Asks out of everyday navigation while retaining an explicit machine-wide view for operators who need it (#1201).

- `lf ls`, `lf roadmap`, and `lf ask list --user` default to the current repository.
- Linked worktrees resolve to their main checkout, so the same repository scope follows work across Loopflow-managed worktrees.
- Add `--all` to any of those commands to restore the previous machine-wide view.

## Hand work to humans with context intact

Task cycles now place human attention at a review surface suited to the kind of work. Local Asks collect in one deduplicated terminal hub, design review carries enough context for someone arriving cold, and Task work is checkpointed and pushed around Ask transitions so a parked decision does not strand local progress (#1203).

- `lf task start <project> "Fix broken behavior" --fix` selects a fix cycle that reaches a working demo before landing.
- `lf task start <project> "Add new behavior" --feature` selects a feature cycle that pauses for design review.
- Projects can make either cycle the default for their Tasks with `cycle: fix` or `cycle: feature` under `## Flows`.
- Explicit `--first`, `--loop`, and `--finally` choices still override cycle presets.
- `lf ask open <ask-id>` presents local Ask sessions through the shared `lf-asks` tmux hub; remote sessions keep their direct presentation path.
- Checkpoint or push failures at Ask boundaries warn without blocking the Ask transition.

## Operational notes

- Repository scoping changes the default output of `lf ls`, `lf roadmap`, and `lf ask list --user`; use `--all` in machine-wide automation.
- Ask boundaries may now create and push checkpoints before parking and after resolution, cancellation, or release.
- Fix cycles include the new `ship-demo` human gate before landing.

## Small changes

- Bare terminal-control launches now preserve valid global options. For example, `lf -m claude` opens terminal control with Claude selected instead of dropping the provider choice (#1200).