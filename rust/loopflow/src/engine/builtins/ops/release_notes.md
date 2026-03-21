Write narrative release notes for {target_name} version {version}.

Do not ask questions. Make best-effort assumptions and proceed.

## Workflow

1. Read the PR list below. Note diff stats — PRs with 100+ additions are major changes.
2. Find connections between PRs. Which ones are part of the same story?
3. For major changes (100+ lines added), run `git diff <prev_tag>..HEAD -- <relevant paths>` to understand what actually changed. The PR title and body are a summary — the diff is ground truth.
4. Write the release notes.

## Output format

Raw markdown only. No JSON, no code fences wrapping the output.

First line must be exactly: `# v{version}`

Structure:
1. **Opening** — 2-3 sentences answering "why upgrade?" Not a list. Not "This release includes..." Tell the story.
2. **Thematic sections** — named after what actually changed in this release. Not "Improvements" or "Bug fixes" — names like "Sandbox execution", "Release workflow", "CLI ergonomics". Each section has a prose paragraph connecting the changes, followed by detail bullets for scanners.
3. **Small changes** — group minor fixes and tweaks at the end under a single section.

## Theming

Theme names come from the changes, not a template. Read the PRs and find what they're about.

When target context is provided (target name, tag prefix, area scope), include only changes relevant to that scope.

## Style

Lead with outcomes, not mechanisms. What can users do now? What got better?

Good: "`lf op next` now starts fresh from main after merged PRs — no manual cleanup"
Bad: "Refactored release.rs to extract helper functions"

Skip internal refactors unless they affect what users experience.

Each thematic section: prose paragraph first (what's the story?), then bullets (what specifically changed?). The prose connects related PRs into a narrative. The bullets give scanners the detail.

If previous release notes are provided below, match their voice and density.
