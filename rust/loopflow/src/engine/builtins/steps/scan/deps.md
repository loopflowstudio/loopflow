---
requires: none
produces: scratch/scan-deps.md
model: claude:sonnet
---
Check dependencies for major version bumps, deprecations, and end-of-life notices.

## Workflow

1. **Find manifests.** Read dependency files in the area:
   - `Cargo.toml` / `Cargo.lock` (Rust)
   - `pyproject.toml` / `uv.lock` / `requirements.txt` (Python)
   - `package.json` / `package-lock.json` (Node)
   - `Package.swift` / `Package.resolved` (Swift)
   - `go.mod` / `go.sum` (Go)

2. **Extract dependencies.** List each direct dependency with its current version.

3. **Check for updates.** For each dependency, search for:
   - Major version releases (2.x → 3.x)
   - Deprecation notices or archival
   - End-of-life announcements
   - Migration guides for breaking changes

4. **Assess impact.** For each finding:
   - Is the current version still maintained?
   - Does the new version have breaking changes?
   - Is there a migration guide?
   - How heavily do we use this dependency?

## Output

Write `scratch/scan-deps.md`:

```markdown
# Dependency Scan — <date>

## Action needed

### <package>: <current> → <latest>
- **Type**: major release | deprecated | end-of-life
- **Breaking changes**: <yes/no — summary if yes>
- **Migration guide**: <URL or "none found">
- **Our usage**: <light | moderate | heavy>
- **Recommendation**: <upgrade now | plan upgrade | monitor | replace>

## Up to date
<Dependencies checked that are current or on latest major>
```

## What to avoid

**Chasing every minor bump.** Focus on major versions, deprecations, and end-of-life. Minor/patch updates are noise for this step.

**Recommending upgrades without context.** A major version bump in a lightly-used utility is different from one in a core framework. Note usage intensity.
