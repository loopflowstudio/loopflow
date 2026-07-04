---
requires: buildable artifact on branch
produces: scratch/run-observations.md
default_agent: codex
action_style: procedural
---
Build and execute the artifact, then record what actually happened.

## Orientation

Read `GOAL.md`, `scratch/`, and the project files needed to understand the
artifact's native run command. This is a headless verification step: execute the
product, observe real behavior, and report pass/fail against the goal.

## Workflow

1. Build the artifact with its native command (`cargo test`, `uv run pytest`,
   `npm test`, `swift test`, or the command the project declares).
2. Execute the artifact in the mode the goal names:
   - CLI: run the binary or command with representative arguments.
   - Server: start it, hit the required endpoint, then stop it.
   - Client/mobile: build and launch the smallest available simulator or smoke
     target.
3. Capture the exact commands, exit codes, and important output.
4. Write `scratch/run-observations.md` with:
   - the commands run
   - observed output
   - whether the behavior satisfies `GOAL.md`
   - any missing primitive that forced manual workaround

## Rules

- Assert on observed behavior, not intent.
- Prefer the project's existing scripts and README commands.
- Keep `.lf/steps/` empty. If a local step is required to run the product,
  record that as a vocabulary gap in `scratch/questions.md`.
- Do not fold this into `demo`: this step is for headless, chainable execution.
