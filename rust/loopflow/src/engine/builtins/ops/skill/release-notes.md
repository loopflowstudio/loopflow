---
requires: LF_RELEASE_NOTES_CONTEXT env var pointing to JSON release context
produces: RELEASE_NOTES.md
diff_files: false
---
Write release notes that fuse release intent with shipped behavior.

## Workflow

1. Read `LF_RELEASE_NOTES_CONTEXT` and parse the JSON. `source_limits` states
   the hard context/output bounds; `omissions` records source material that did
   not fit. Never infer behavior from omitted source.
2. Treat `decisions` as the intent ledger: what changed in product direction, release policy, operations, and user experience during this cycle.
3. Treat `commits` as the bounded behavior ledger for the exact first-parent
   git range in the target area. Use `merged_prs` to recover intent and
   discussion. Do not expand the source with git or GitHub queries; the bounded
   JSON is the complete notes input.
4. Fuse them. The release notes should explain what users, operators, and contributors can do differently now. Use commits/PRs to ground every claim.
5. Keep the previous release-note voice if `previous_release_notes` exists, but improve structure when the previous notes were too raw.
6. Write `RELEASE_NOTES.md`.

## Output

Raw markdown only. No JSON. No code fence wrapping the output. Keep the
complete file below `source_limits.release_notes_bytes`; Loopflow rejects
oversized notes because this file also becomes release-queue metadata.

First line must be exactly:

```markdown
# v<version>
```

Structure:

1. **Opening story** — 2-4 sentences. Answer “why upgrade?” and name the through-line of the release. This is narrative, not a list.
2. **Thematic sections** — sections named after user/operator outcomes, not implementation buckets. Each section starts with a short paragraph connecting the decisions to the shipped behavior, followed by concise bullets for scanners.
3. **Operational notes** — include only when relevant: release process, deployment, migration, billing, TestFlight, compatibility, or known manual steps.
4. **Small changes** — minor fixes and polish that do not deserve a full theme.

## Source handling

- Decisions are source material, not release notes. Do not paste the ledger wholesale.
- PR titles are source material, not release notes. Do not dump them chronologically.
- If decisions and commits disagree, trust the commits for what shipped and use decisions to explain why.
- If decisions mention future work that did not ship, either omit it or mark it clearly as “not included.”
- If `omissions` contains non-zero counts, synthesize only from the included
  evidence. Mention the bounded sample only when it materially limits a claim;
  do not dump the omission ledger into the notes.
- If there are no decisions, build the narrative from commits, merged PRs, and
  diffs.
- If there are decisions but no matching shipped behavior, keep the note cautious: describe the policy/intent change, not an implementation that is absent.

## Style

- Lead with outcomes, not mechanisms.
- Be specific and factual. No marketing filler.
- Prefer a few strong themes over many headings.
- Write for the person deciding whether to upgrade and the operator debugging the release six weeks later.
- Synthesize from the merged PRs and their intent; `RELEASE_NOTES.md` is the interpreted story, not a changelog.

## Quality bar

A good release note could not be generated from PR titles alone. It reads the
intent from decisions and merged PR descriptions, proves it against the exact
commits being tagged, and leaves a concise operational record.
