---
requires: LF_RELEASE_NOTES_CONTEXT env var pointing to JSON release context
produces: RELEASE_NOTES.md
diff_files: false
---
Write narrative release notes from structured release context.

## Workflow

1. Read `LF_RELEASE_NOTES_CONTEXT` and parse the JSON.
2. If `decisions` is present, treat it as primary source — the agent-authored ledger of intent decisions made during this cycle. Use its themes, voice, and framing. PRs in `merged_prs` are supplementary: use them to fill gaps and surface mechanical changes that weren't logged as decisions.
3. If `decisions` is absent or empty, fall back to `merged_prs` as the primary source.
4. Keep the previous release-note voice if `previous_release_notes` exists.
5. Write `RELEASE_NOTES.md` with:
   - `# v<version>` header
   - "Changes since `<prev_tag>`" section
   - themed sections grouping by what users feel, not what code changed
   - concise bullets with concrete impact

## Guardrails

- Decisions describe intent; PRs describe diffs. Lead with intent.
- Don't dump PR titles verbatim — rephrase for users.
- Prefer clear themes over chronological lists.
- Keep language specific and factual; no marketing filler.
- Always overwrite `RELEASE_NOTES.md` with the new version's notes.
