# 04: Sandbox Integration and Validation

**Status (2026-02-28):** Experimental track. Host validation passes on Docker Sandbox CLI v0.12.0, but DinD validation is blocked because the bundled lfd image lacks the sandbox CLI plugin.

**Finish line:** Sandbox executor runs Claude end-to-end on macOS (self-hosted + Concerto), and cleanup works across all lifecycle events.

## What we're trying to learn

Does sandbox mode work transparently — same streaming, same context files, same cleanup — across macOS local, macOS Concerto, and Linux? Where does it break?

## What's built

Validation infrastructure is shipped. Three layers:

- **Rust unit tests** (`sandbox_` module in `rust/loopflow/src/lfd/executor/sandbox.rs`) — command shape, streaming, cleanup on timeout, orphan recovery. Uses fake docker scripts via PATH interception (not trait extraction — keeps production code simple).
- **Platform script** (`scripts/test_sandbox_platforms.sh`) — full sandbox lifecycle on real hosts. Startup probe, context file sync, cleanup verification. Fails fast when required CLI commands aren't available.
- **DinD command** (`scripts/concerto-dev.py sandbox-dind`) — probes sandbox lifecycle inside bundled lfd container.
- **CI job** (`sandbox-smoke` in `.github/workflows/ci.yml`) — probe-gate pattern: skips when sandbox plugin unavailable, activates automatically when it lands on CI runners.

## What's blocked

Concerto DinD validation remains blocked because the bundled lfd container does not have a sandbox-capable Docker CLI (`docker sandbox` is not recognized).

CI `sandbox-smoke` remains probe-gated. It activates only when runners expose required sandbox capabilities (`create`, `exec`, `rm`, `ls`, and `claude` template support).

## Remaining scope

### Context and workspace sync

Context files written to `.lf/prompts/<step>.context.md` in the worktree should be visible immediately inside sandbox runs. Validate host↔sandbox file visibility in the platform script.

### Cleanup

Sandbox cleanup works on:

- Normal completion (`docker sandbox rm` after run)
- Terminate (`docker sandbox rm` via active map)
- Startup janitor: `docker sandbox ls`, match `lf-*` prefix, fail orphaned DB runs, remove orphaned sandboxes

No stream rehydration in phase 1.

### Concerto DinD

Verify `docker sandbox` commands work from inside the bundled lfd container with `/var/run/docker.sock` mounted. Confirm the lfd Docker image includes the sandbox CLI plugin.

DinD probe rerun on 2026-02-28 against a running test container; command failed with `docker: 'sandbox' is not a docker command`. Re-run after sandbox plugin distribution is solved for Linux containers.

### Platform validation

- **macOS (self-hosted):** full validation — probe, run, stream, cleanup, fallback
- **macOS (Concerto):** bundled daemon path with sandbox executor
- **Linux:** smoke validation (experimental) — probe + single run + cleanup

## Done when

- Platform validation script passes on macOS
- Context file sync verified via platform script
- Concerto DinD path validated or documented as blocked with evidence
