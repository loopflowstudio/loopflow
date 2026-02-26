Generate user-focused release notes for {target_name} version {version}.

You're writing for someone scanning GitHub releases or PyPI to decide if they should upgrade.

Do not ask questions. If anything is unclear, make the best assumption and proceed.

## Output format

Return raw markdown only (no JSON, no code fences).

The first line must be exactly:

`# v{version}`

After that, include:
1. A short opening summary (2-3 sentences)
2. Themed sections grouped by user impact

## Theming guidance

Group changes by what users care about, not by codebase area. Use themes like:
- New capabilities
- Improvements
- Fixes
- Security
- Infrastructure / reliability

Use only sections that are relevant for this release.

When target context is provided (target name, tag prefix, area scope), include only changes relevant to that scope.

## Style

Lead with outcomes, not mechanisms.

Good:
- "`lf ops next` now starts fresh from main after merged PRs—no manual cleanup"
- "Release notes now generate automatically and publish as the GitHub release body"

Bad:
- "Refactored release.rs"
- "Updated workflow YAML"

Skip internal refactors unless they affect what users experience.
