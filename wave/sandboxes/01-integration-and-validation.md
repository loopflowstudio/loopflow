# 01: Integration and Validation

**Finish line:** Sandbox executor runs Claude end-to-end on macOS (self-hosted + Concerto), Gemini path passes automated smoke test, and cleanup works across all lifecycle events.

## What we're trying to learn

Does sandbox mode work transparently — same streaming, same context files, same cleanup — across macOS local, macOS Concerto, and Linux? Where does it break?

## Known risk: CLI compatibility

The implementation assumes `docker sandbox create`, `docker sandbox exec`, and `docker sandbox stop` exist. On this machine (February 27, 2026), `docker sandbox --help` only advertises `run/ls/inspect/rm/version`. The startup probe catches missing subcommands, but we need to confirm the minimum Docker Sandbox plugin version that supports the full lifecycle — or adapt the command strategy if `create`/`exec` don't land.

## Scope

### Context and workspace sync

Context files written to `.lf/logs/<step>.context.md` in the worktree. Sandbox workspace mount uses same absolute path. Verify host↔sandbox file visibility is immediate.

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

### Smoke test

Gemini path covered by automated smoke test (no manual test required). Add `sandbox_` test module alongside existing `docker_` tests.

## Done when

- CLI compatibility confirmed: minimum Docker Sandbox plugin version identified, or command strategy adapted
- Context files readable inside sandboxed Claude runs
- Gemini CLI confirmed working inside `claude` sandbox template (or custom template strategy documented)
- Gemini path covered by automated smoke test
- Sandbox cleanup works on completion, terminate, and startup janitor
- macOS validated (self-hosted + Concerto path)
- Linux smoke validated (experimental)
- Concerto bundled daemon path validated with sandbox executor (or documented as blocked with evidence)
