# Gemini Template Evidence

## What we're testing

Whether Gemini CLI is available inside the `claude` Docker Sandbox template, and if not, whether it can be installed at runtime.

## How to validate

Run the platform validation script and capture the Gemini section:

```bash
scripts/test_sandbox_platforms.sh 2>&1 | tee scratch/gemini-validation-output.txt
```

Or probe manually:

```bash
docker sandbox create --name lf-gemini-test claude /tmp
docker sandbox exec lf-gemini-test -- which gemini 2>/dev/null || echo "NOT FOUND"
docker sandbox exec lf-gemini-test -- gemini --version 2>/dev/null || echo "NOT AVAILABLE"
docker sandbox rm lf-gemini-test
```

## Results

**Status:** BLOCKED (sandbox CLI lifecycle incompatible on this host)

**Date:** 2026-02-28
**Sandbox template version:** Unknown (could not run `docker sandbox create`)

### Outcome

| Outcome | Evidence | Next step |
|---------|----------|-----------|
| Gemini CLI present | N/A (probe blocked before sandbox startup) | Re-run once required lifecycle commands are available |
| Not present, installable via npm | N/A (probe blocked before sandbox startup) | Re-run once required lifecycle commands are available |
| Not present, install blocked | N/A (probe blocked before sandbox startup) | Re-run once required lifecycle commands are available |

### Raw output

```
=== Platform ===
  OS:      Darwin arm64
  Docker:  28.5.2
  Sandbox: github.com/docker/sandboxes/cli-plugin v0.6.0 6e5a0050c0891d260905b7771493674060636b07
  FAIL: sandbox plugin missing required commands: create exec
```
