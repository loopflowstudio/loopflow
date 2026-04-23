---
requires: LF_RELEASE_NOTES_CONTEXT env var pointing to JSON release context
produces: RELEASE_NOTES.md
diff_files: false
---
Write narrative release notes from structured release context.

## Workflow

1. Read `LF_RELEASE_NOTES_CONTEXT` and parse the JSON.
2. Keep the previous release-note voice if `previous_release_notes` exists.
3. Write `RELEASE_NOTES.md` with:
   - `# v<version>` header
   - "Changes since `<prev_tag>`" section
   - themed sections that group the merged PRs by what users feel
   - concise bullets with concrete impact (not PR-title dumps)

## Guardrails

- Use real merged PR data from the context file.
- Prefer clear themes over chronological lists.
- Keep language specific and factual; no marketing filler.
- Always overwrite `RELEASE_NOTES.md` with the new version's notes.
