---
requires: none
produces: scratch/scan-cves.md
model: claude:sonnet
---
Check dependencies for known security vulnerabilities.

## Workflow

1. **Find manifests.** Read dependency files in the area:
   - `Cargo.toml` / `Cargo.lock` (Rust)
   - `pyproject.toml` / `uv.lock` / `requirements.txt` (Python)
   - `package.json` / `package-lock.json` (Node)
   - `Package.swift` / `Package.resolved` (Swift)
   - `go.mod` / `go.sum` (Go)

2. **Extract dependencies.** List each direct dependency with its pinned or constrained version.

3. **Search for CVEs.** For each dependency, search for recent security advisories:
   - Use web search: `"<package-name>" CVE OR security advisory OR vulnerability`
   - Check severity (critical, high, medium, low)
   - Check whether the pinned version is affected
   - Check whether a patched version exists

4. **Write findings.** Only report vulnerabilities that affect versions in use.

## Output

Write `scratch/scan-cves.md`:

```markdown
# CVE Scan — <date>

## Critical / High

### <package> — <CVE-ID>
- **Severity**: <critical|high>
- **Current version**: <version>
- **Affected versions**: <range>
- **Fixed in**: <version>
- **Summary**: <one line>
- **Source**: <URL>

## Medium / Low

### <package> — <CVE-ID>
...

## Clean
<List of dependencies checked with no known issues>
```

If no vulnerabilities are found, write that explicitly — a clean scan is a useful result.

## What to avoid

**False positives.** Verify the pinned version is actually in the affected range before reporting.

**Stale data.** Prefer recent sources. An advisory from 2019 for a dependency you're running at v3.x is probably irrelevant.

**Noise.** Skip dev-only dependencies unless the advisory is critical severity.
