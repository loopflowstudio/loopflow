---
name: scan-scan-report
description: Scan dependencies and external APIs for vulnerabilities, staleness, and breaking changes.
loopflow: true
disable-model-invocation: true
---
Scan dependencies and external APIs for vulnerabilities, staleness, and breaking changes.

## Workflow

1. **Find manifests.** Read dependency files in the area:
   - `Cargo.toml` / `Cargo.lock` (Rust)
   - `pyproject.toml` / `uv.lock` / `requirements.txt` (Python)
   - `package.json` / `package-lock.json` (Node)
   - `Package.swift` / `Package.resolved` (Swift)
   - `go.mod` / `go.sum` (Go)
   - `Gemfile` / `Gemfile.lock` (Ruby)
   - `build.gradle` / `pom.xml` (Java/Kotlin)
   - `*.csproj` / `Directory.Packages.props` (C#/.NET)
   - `composer.json` / `composer.lock` (PHP)
   - `pubspec.yaml` / `pubspec.lock` (Dart/Flutter)

2. **Extract dependencies.** List each direct dependency with its pinned or constrained version.

3. **Check for CVEs.** For each dependency, search for recent security advisories:
   - Use web search: `"<package-name>" CVE OR security advisory OR vulnerability`
   - Check severity (critical, high, medium, low)
   - Verify the pinned version is in the affected range
   - Check whether a patched version exists

4. **Check for staleness.** For each dependency, search for:
   - Major version releases (2.x -> 3.x)
   - Deprecation notices or archival
   - End-of-life announcements

5. **Check infra dependencies.** Look for version-pinned infra manifests:
   - `Dockerfile` / `docker-compose.yml` (base image versions)
   - `*.tf` / `.terraform.lock.hcl` (Terraform providers/modules)
   - `.github/workflows/*.yml` (GitHub Actions versions)
   - `Chart.yaml` (Helm chart dependencies)

6. **Identify external APIs.** Scan the area for:
   - API client code (REST calls, SDK usage, GraphQL queries)
   - Service integrations (cloud providers, SaaS APIs, webhooks)
   - SDK imports (e.g., `anthropic`, `openai`, `stripe`, `aws-sdk`)

7. **Check upstream changes.** For each external API or service, search for:
   - API version deprecations or sunset dates
   - Breaking changes in recent releases
   - Migration guides or upgrade paths

## Output

Write `scratch/scan-report.md`:

```markdown
# Scan Report — <date>

## Vulnerabilities

### <package> — <CVE-ID>
- **Severity**: critical | high | medium | low
- **Current version**: <version>
- **Fixed in**: <version>
- **Summary**: <one line>
- **Source**: <URL>

## Stale dependencies

### <package>: <current> -> <latest>
- **Type**: major release | deprecated | end-of-life
- **Breaking changes**: <yes/no — summary if yes>
- **Migration guide**: <URL or "none found">
- **Our usage**: light | moderate | heavy

## Upstream changes

### <service/API> — <change summary>
- **What changed**: <description>
- **Affects**: <which files/features in our code>
- **Deadline**: <deprecation date, if any>
- **Migration**: <URL or summary>

## Clean
<Dependencies and services checked with no issues>
```

If a section has no findings, say so explicitly — a clean scan is a useful result.

## What to avoid

**False positives.** Verify pinned versions are actually in affected ranges before reporting CVEs.

**Noise.** Skip dev-only dependencies unless critical severity. Skip minor/patch version bumps. Only surface upstream changes that affect code in the area.

**Missing context.** Connect every finding to specific code. A CVE in a package we import differently than the affected path is not a finding. An API deprecation only matters if we call that endpoint.
