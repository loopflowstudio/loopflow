---
requires: GOAL.md describing the target surface
produces: runnable skeleton, initialized repo, scratch/<branch>.md
default_agent: codex
action_style: procedural
---
Stand up a greenfield project from the goal alone.

## Orientation

Start from the current directory. Treat `GOAL.md` as the product brief and do
not ask for more input. If the directory is not a git repo, initialize one
before you finish this step:

```bash
git init -b main 2>/dev/null || git init
git checkout -B main
```

If `main` already exists or the repo already has a branch, keep the existing
branch. Do not create a second implementation path or compatibility shim.

## Goal

Create the smallest idiomatic skeleton that builds and runs. The skeleton should
make the product surface visible, but it should not implement speculative
features beyond what `GOAL.md` asks for.

## Workflow

1. Read `GOAL.md`.
2. Pick the simplest conventional toolchain for the requested surface:
   - CLI: Rust/Cargo, Python/uv, Node/npm, or the toolchain named in the goal.
   - Server: the smallest framework that can expose the requested endpoint.
   - Client/mobile: the native project skeleton the goal names.
3. Create the project files needed to compile and run.
4. Add a minimal smoke test or equivalent if the chosen toolchain has one.
5. Run the build and the smallest execution command.
6. Write `scratch/<branch>.md` as a short implementation brief copied from
   `GOAL.md`: what exists, what remains, and the exact command that should prove
   the target works.

## Rules

- `GOAL.md` is the brief. Do not start an interactive design conversation.
- Keep the generated product boring and idiomatic.
- Keep `.lf/steps/` empty. If you need to author a new step to proceed, the
  language failed; record the missing primitive in `scratch/questions.md`.
- Do not vendor secrets, credentials, or machine-local config.
- Leave the directory in a state where the next flow step can commit normally.
