Write narrative release notes for {target_name} version {version}.

Do not ask questions. Make best-effort assumptions and proceed.

## Workflow

1. Read the structured release context. Use release decisions as the intent ledger and PRs/diffs as the behavior ledger.
2. Find the through-line: which decisions explain why these changes belong in one release?
3. For major changes or unclear PRs, run `git diff <prev_tag>..HEAD -- <relevant paths>` to verify what actually shipped.
4. Write the release notes as an interpreted story grounded in shipped behavior.

## Output format

Raw markdown only. No JSON, no code fences wrapping the output.

First line must be exactly: `# v{version}`

Structure:
1. **Opening** — 2-4 sentences answering “why upgrade?” Tell the release story.
2. **Thematic sections** — named after user/operator outcomes, not implementation buckets. Each section has a prose paragraph connecting intent to behavior, followed by detail bullets for scanners.
3. **Operational notes** — only when relevant: deployment, release process, migration, billing, TestFlight, compatibility, or manual steps.
4. **Small changes** — group minor fixes and tweaks at the end.

## Theming

Theme names come from the release, not a template. Read decisions first, then PRs and diffs. Use decisions to understand intent; use commits and diffs to prove what shipped.

When target context is provided (target name, tag prefix, area scope), include only changes relevant to that scope.

## Style

Lead with outcomes, not mechanisms.

Good: “`lf release run` now generates versioned notes from the merged PRs and archives them for audit.”
Bad: “Updated release.rs and release_notes.md.”

Do not dump PR titles verbatim. The release notes are the synthesis of what shipped and why, drawn from the PRs themselves.

Skip internal refactors unless they affect what users experience or how operators run the system.

If previous release notes are provided below, match their voice and density while keeping this release concise.
