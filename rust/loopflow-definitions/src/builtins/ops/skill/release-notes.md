---
requires: LF_RELEASE_NOTES_CONTEXT env var pointing to JSON release context
produces: RELEASE_NOTES.md
diff_files: false
---
Write release notes that tie why the release changed to what actually shipped.

## Workflow

1. Read the JSON at `LF_RELEASE_NOTES_CONTEXT`. `source_limits` gives the
   context and output limits; `omissions` counts source that didn't fit. Never
   guess at behavior from omitted source.
2. `decisions` says what changed this cycle in product direction, release
   policy, operations and user experience.
3. `commits` is what shipped: the exact first-parent range for the target
   area. `merged_prs` explains why. The JSON is the whole input; don't query
   git or GitHub for more.
4. Combine them: what users, operators and contributors can do differently
   now. Ground every claim in a commit or PR.
5. Keep the voice of `previous_release_notes` if present, but fix structure
   when they were too raw.
6. Write to `LF_RELEASE_NOTES_OUTPUT` when set, otherwise `RELEASE_NOTES.md`.
   A preview passes a temporary path; don't edit the repository's release
   files or publish anything.

## Output

Raw markdown only. No JSON, no code fence around it. Stay under
`source_limits.release_notes_bytes`; Loopflow rejects bigger notes because the
file also becomes release-queue metadata.

First line must be exactly:

```markdown
# v<version>
```

Structure:

1. **Opening story**: 2-4 sentences of narrative, not a list. Why upgrade, and
   what ties the release together.
2. **Thematic sections**: named after outcomes, not implementation buckets.
   Each opens with a short paragraph linking the decisions to what shipped,
   then concise bullets.
3. **Operational notes**: only when needed: release process, deployment,
   migration, billing, TestFlight, compatibility, manual steps.
4. **Small changes**: minor fixes and polish.

## Sources

- Decisions and PR titles are inputs, not notes. Don't paste the ledger or
  list PRs in order.
- When decisions and commits disagree, commits say what shipped; decisions
  say why.
- Future work that didn't ship: leave it out or mark it "not included."
- With non-zero `omissions`, write only from what's included. Mention the
  sample limit only when it weakens a specific claim.
- With no decisions, build the story from commits, merged PRs and diffs.
- Minor notes cover the whole cycle since the preceding `.0` tag. Some
  features already shipped in patches; describe progress across the cycle,
  reconcile later changes with earlier ones, and describe the final behavior.
- Decisions with no shipped behavior: describe the policy or intent change,
  not an implementation that isn't there.

## Style

- Outcomes first, mechanisms second.
- Specific and factual. No marketing filler.
- A few strong themes beat many headings.
- Write for someone deciding whether to upgrade, and for the operator
  debugging the release six weeks later.
- `RELEASE_NOTES.md` is the interpreted story, not a changelog. Good notes
  could not be written from PR titles alone.
