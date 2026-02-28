# 01: Integration and Validation

**Finish line:** Sandbox executor runs Claude end-to-end on macOS (self-hosted + Concerto), Gemini path passes automated smoke test, and cleanup works across all lifecycle events.

## What we're trying to learn

Does sandbox mode work transparently — same streaming, same context files, same cleanup — across macOS local, macOS Concerto, and Linux? Where does it break?

## Scope

### Context and workspace sync

Context files written to `.lf/logs/<step>.context.md` in the worktree should be visible immediately inside sandbox runs. Validate host↔sandbox file visibility in the platform script.

### Cleanup

Sandbox cleanup works on:

- Normal completion (`docker sandbox rm` after run)
- Terminate (`docker sandbox rm` via active map)
- Startup janitor: `docker sandbox ls`, match `lf-*` prefix, fail orphaned DB runs, remove orphaned sandboxes

No stream rehydration in phase 1.

### Concerto DinD

Verify `docker sandbox` commands work from inside the bundled lfd container with `/var/run/docker.sock` mounted. Confirm the lfd Docker image includes the sandbox CLI plugin.

### Platform validation

- **macOS (self-hosted):** full validation — probe, run, stream, cleanup, fallback
- **macOS (Concerto):** bundled daemon path with sandbox executor
- **Linux:** smoke validation (experimental) — probe + single run + cleanup

### Gemini template validation

Phase 1 assumes Gemini CLI works inside the `claude` sandbox template. Validate this — if Gemini CLI isn't pre-installed, determine whether `docker sandbox exec` can install it or we need a custom template.

## Done when

- Sandbox cleanup works on completion, terminate, and startup janitor
- Context files readable inside sandboxed Claude runs
- macOS validated (self-hosted + Concerto path)
- Linux smoke validated (experimental)
- Concerto bundled daemon path validated with sandbox executor (or documented as blocked with evidence)
- Gemini CLI confirmed working inside `claude` sandbox template (or custom template strategy documented)
