# Sandbox Integration and Validation

## Problem

The sandbox executor and adaptive routing work — command compatibility is covered in unit tests, context logic exists, and the startup probe gates correctly. But we haven't proven the full lifecycle across real deployment surfaces. Cleanup edge cases, Concerto's DinD path, platform differences, and Gemini template compatibility are all untested assumptions.

This is the last gate before Phase 2 (full agent rollout and Bollard removal). Every assumption validated here is one less surprise when we cut the fallback.

## Approach

Hybrid validation: Rust integration tests for testable lifecycle scenarios, validation scripts for platform-specific surfaces that can't run in standard CI.

### 1. Cleanup tests (Rust `sandbox_` module)

Already implemented in `rust/loopflow/src/lfd/executor/sandbox.rs` (lines 454–556). Three scenarios covered:

- **Normal completion:** `sandbox_executor_runs_agent_and_streams_output` — verifies create → exec → rm sequence via fake docker script
- **Timeout cleanup:** `sandbox_executor_timeout_stops_and_removes_sandbox` — verifies stop + rm on timeout
- **Orphan recovery:** `sandbox_recovery_removes_only_managed_sandboxes` — verifies `lf-*` prefix filtering via inspect

Tests use a fake docker shell script (PATH interception) rather than trait extraction. This avoids reshaping production code for tests and keeps the sandbox executor's internal structure simple.

### 2. Concerto DinD validation

The Concerto app bundles lfd in a Docker container with `/var/run/docker.sock` mounted. For sandbox mode to work inside this container, two things must be true:

1. The `docker` CLI is installed in the lfd container image
2. The `docker-sandbox` CLI plugin is available (installed or mounted)

**Validation approach:** Added `sandbox-dind` command to `scripts/concerto-dev.py`:

```bash
uv run python scripts/concerto-dev.py sandbox-dind --container lfd-container
```

If the sandbox plugin isn't in the image, two options:
- **Option A (chosen):** Add `docker-sandbox` plugin to the lfd Dockerfile. This is a build-time dependency, not a runtime discovery.
- **Option B:** Fall back to DockerExecutor when running inside DinD. This works but defeats the purpose of sandbox mode for Concerto users.

Write the result (pass/blocked with evidence) to `scratch/dind-evidence.md`.

### 3. Platform validation script

Create `scripts/test_sandbox_platforms.sh` — a single script that runs the full sandbox lifecycle and reports results. Designed to run manually on each target platform.

```bash
#!/usr/bin/env bash
# Usage: scripts/test_sandbox_platforms.sh
# Run on each target: macOS (native), macOS (Concerto/DinD), Linux

set -euo pipefail

echo "=== Sandbox Platform Validation ==="
echo "Platform: $(uname -s) $(uname -m)"
echo "Docker: $(docker version --format '{{.Client.Version}}')"
echo "Sandbox: $(docker sandbox version 2>&1 || echo 'NOT AVAILABLE')"

# 1. Startup probe
docker sandbox create --name lf-platform-test claude /tmp
docker sandbox exec lf-platform-test -- echo "probe ok"

# 2. Context file visibility
mkdir -p /tmp/lf-test-workspace/.lf/logs
echo "test context" > /tmp/lf-test-workspace/.lf/logs/test.context.md
docker sandbox create --name lf-ctx-test claude /tmp/lf-test-workspace
docker sandbox exec lf-ctx-test -- cat .lf/logs/test.context.md

# 3. Cleanup
docker sandbox rm lf-platform-test
docker sandbox rm lf-ctx-test

# 4. Verify cleanup
remaining=$(docker sandbox ls --quiet | grep "^lf-" || true)
if [ -n "$remaining" ]; then
  echo "FAIL: orphaned sandboxes remain: $remaining"
  exit 1
fi

echo "=== PASS ==="
```

Run on:
- **macOS self-hosted:** Full validation. This is the primary target.
- **macOS Concerto:** Run from inside the lfd container (DinD path).
- **Linux CI:** Add to `docker-smoke` CI job as an optional step. If sandbox plugin isn't available on Linux runners, skip gracefully and log.

### 4. Gemini template validation

The `claude` sandbox template is the only template Docker Sandbox ships. Gemini CLI may or may not be pre-installed.

**Validation:**
```bash
docker sandbox create --name lf-gemini-test claude /tmp
docker sandbox exec lf-gemini-test -- which gemini 2>/dev/null || echo "NOT FOUND"
docker sandbox exec lf-gemini-test -- gemini --version 2>/dev/null || echo "NOT AVAILABLE"
docker sandbox rm lf-gemini-test
```

Three possible outcomes and their responses:

| Outcome | Response |
|---------|----------|
| Gemini CLI present and works | Done. Document minimum template version. |
| Not present, `exec` can install it | Add install step to sandbox executor's Gemini harness path: `docker sandbox exec <id> -- npm install -g @google/gemini-cli` before the main exec. |
| Not present, install blocked | Investigate custom template creation: `docker sandbox create --template custom-lf`. If custom templates aren't supported yet, document as blocked for Phase 2. |

Add the Gemini probe to `scripts/test_sandbox_platforms.sh` so it runs alongside platform validation.

### 5. CI integration

Add a `sandbox-smoke` job to `.github/workflows/ci.yml`:

```yaml
sandbox-smoke:
  runs-on: ubuntu-latest  # or macos-latest if sandbox needs macOS
  steps:
    - uses: actions/checkout@v4
    - name: Check sandbox availability
      id: probe
      run: docker sandbox version && echo "available=true" >> "$GITHUB_OUTPUT" || echo "available=false" >> "$GITHUB_OUTPUT"
    - name: Run sandbox tests
      if: steps.probe.outputs.available == 'true'
      run: cargo test -p loopflow sandbox
    - name: Platform validation
      if: steps.probe.outputs.available == 'true'
      run: scripts/test_sandbox_platforms.sh
```

The probe-gate pattern means CI doesn't fail when the sandbox plugin isn't available — it skips gracefully. This matches the adaptive executor's runtime behavior.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| All-Rust integration tests | Stronger CI guarantees, but can't test DinD or cross-platform without specialized runners | Platform validation needs real environments, not mocks |
| All-script validation | Easy to run anywhere, but no CI regression safety | Cleanup and orphan recovery logic should be tested in Rust where the implementation lives |
| Skip Gemini until Phase 2 | Less scope now, but Phase 2 is "all harnesses through sandbox" — Gemini blockers discovered late would delay Bollard removal | Small investigation now prevents large surprise later |
| Custom sandbox template from day 1 | Full control over what's installed, but template API may not be stable | Template creation may not be supported yet. Validate default first, custom only if needed. |

## Key decisions

**Fake docker script over trait extraction.** The sandbox executor's tests use a fake docker shell script via PATH interception. This is simpler than the docker module's `DockerRecoveryBackend` trait because sandbox recovery is `list → filter → rm`, not the multi-step rehydration docker needs. No production code reshaped for tests.

**Validation script over manual checklist.** One command to run, one environment to verify in. Evidence is captured in output, not in someone's memory.

**Probe-gate CI.** Sandbox tests skip when the plugin isn't available rather than failing. This avoids blocking unrelated PRs while sandbox support rolls out across CI runners.

**Gemini: try default template first.** Don't build custom template infrastructure before confirming it's needed. If `gemini` is already in the `claude` template, that's zero work.

## Scope

- **In scope:** Cleanup tests, DinD validation, platform validation script, Gemini template probe, CI job
- **Out of scope:** Stream rehydration (Phase 2), custom template creation (only if validation proves it necessary), Bollard removal, Codex/OpenCode sandbox routing

## Done when

```bash
# Rust tests pass
cargo test -p loopflow sandbox

# Platform validation passes on macOS
scripts/test_sandbox_platforms.sh

# Gemini outcome documented
cat scratch/gemini-template-evidence.md

# DinD outcome documented
cat scratch/dind-evidence.md
```

- `sandbox_` test module passes in CI (or skips gracefully when sandbox unavailable)
- Cleanup verified: completion, terminate, startup janitor
- macOS self-hosted validated via script
- Concerto DinD path validated or documented as blocked with evidence
- Linux smoke validated or documented as experimental
- Gemini CLI status in `claude` template confirmed with next-step decision

## Current validation snapshot (2026-02-28)

- `cargo test -p loopflow sandbox -- --nocapture` passes.
- Platform script currently blocks on local Docker Sandbox plugin compatibility:
  - `docker sandbox version`: `v0.6.0`
  - Missing required commands: `create`, `exec` (only `run/ls/inspect/rm/version` exposed)
- DinD validation blocked locally because host Docker daemon is not reachable.
