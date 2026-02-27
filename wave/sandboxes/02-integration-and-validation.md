# 02: Integration and Validation

Verify the sandbox executor works end-to-end across platforms and integration points.

## What we're trying to learn

Does sandbox mode work transparently — same streaming, same context files, same cleanup — across macOS local, macOS Concerto, and Linux? Where does it break?

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

### Smoke test

Gemini path covered by automated smoke test (no manual test required). Extend existing `cargo test -p loopflow docker_` or add `sandbox_` test module.

## Done when

- Context files readable inside sandboxed Claude runs
- Gemini path covered by automated smoke test
- Sandbox cleanup works on completion, terminate, and startup janitor
- macOS validated (self-hosted + Concerto path)
- Linux smoke validated (experimental)
- Concerto bundled daemon path validated with sandbox executor (or documented as blocked with evidence)
