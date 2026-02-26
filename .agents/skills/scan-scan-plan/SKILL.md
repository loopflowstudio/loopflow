---
name: scan-scan-plan
description: Turn scan findings into an actionable design doc for implementation.
loopflow: true
disable-model-invocation: true
---
Turn scan findings into an actionable design doc for implementation.

## Workflow

1. **Read the scan report.** Load `scratch/scan-report.md`. If it doesn't exist, stop — there's nothing to plan from.

2. **Triage findings.** For each item in the report, assess:
   - **Urgency**: Is there a deadline, active exploit, or breaking change already live?
   - **Effort**: Is this a version bump in `Cargo.toml`, or a migration across multiple files?
   - **Risk**: Could this change break something? What's the blast radius?

3. **Group into changes.** Cluster related findings into concrete changes:
   - A CVE fix and a major version bump in the same package are one change
   - Multiple API migrations in the same service client are one change
   - Unrelated findings stay separate

4. **Order by priority.** Sequence the changes:
   - Security vulnerabilities first (critical > high > medium)
   - Breaking upstream changes with deadlines next
   - Stale dependencies last

5. **Write the design doc.** For each change, specify what to do — files to touch, versions to target, tests to update.

## Output

Write `scratch/<branch>.md`:

```markdown
# Scan fixes

## Changes

### 1. <short description>
- **Why**: <CVE-ID / deprecation / upstream breaking change>
- **What**: <specific files and changes>
- **Risk**: <what could break, how to verify>

### 2. <short description>
...

## Out of scope
<Findings from the report that don't warrant action now, with brief rationale>
```

## What to avoid

**Boiling the ocean.** Not every finding needs a fix in this PR. Deprioritize low-severity items and things without deadlines. Put them in "out of scope" so they're tracked but not blocking.

**Vague actions.** "Upgrade package X" is not a plan. "Bump X from 2.3 to 3.0 in Cargo.toml, update the `foo::bar` import in src/lib.rs, run tests" is a plan.

**Ignoring test impact.** If a dependency change affects test fixtures, mocks, or CI config, call it out.
