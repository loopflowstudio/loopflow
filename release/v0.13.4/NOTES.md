# v0.13.4

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.4 reduces the work between reopening Desktop and seeing your saved workspace, and gives ongoing conversations the procedures to keep started Tasks moving. Release recovery also preserves a pending version when preparation needs correction, so a rejected candidate no longer consumes another version number.

## Return to your workspace sooner

Desktop avoids repeated model work before its first frame: it reuses resolved repository identities, indexes Sessions by Task, and waits to build the unopened Portfolio window's model until that window renders.

- Returning launches reached their first frame in a median 617 ms, down from 850 ms, across 20 launches per version on a heavily loaded host. CPU use before that frame fell by about a third.
- The sidebar and last selected Task still appear from the saved workspace without a loading state.
- The 400 ms first-frame target remains unmet on that host. These measurements do not establish performance without a saved workspace or with cold OS file caches.

## Keep started work moving in the conversation

Repository, Wave, and Task conversations now include their matching operating procedures. During a turn, they are instructed to account for every started Task, continue eligible idle work, and explain what prevents the rest from moving.

- Check associated Flows and unfinished Execs for live drivers before continuing work, and inspect failures before retrying.
- Preserve review decisions and leave unstarted backlog untouched. A finished Flow or published PR does not by itself mean the Task is complete.
- Answer questions and capture ideas before resuming operation, then group reports into work waiting on the participant, moving, and stuck, with PR links.
- Add `task/session` as a plain skill, with each scope's operating procedure maintained in one source.

## Correct preparation without losing the release version

Recovery previously advanced from v0.13.1 to v0.13.2 after integration introduced a migration absent from the prepared tree, leaving a publication gap. Manual and scheduled recovery now rebuild untagged candidates at the pending version.

- Build a retry branch from the rejected commit while retaining the same version.
- Append migration preparation as another batch in that version, preserving earlier SQL bytes.
- Report recovery blockers for tagged unfinished releases instead of silently skipping ahead. Partial or unknown publication still blocks replacement.

## Operational notes

Install the updated binary and replace existing conversations with `lf session replace <id>` to receive the revised session prompts. Conversations operate only during a turn; this release adds no scheduler. Prompt composition and scenario checks passed, but installed conversations performing mutations remain unverified in the supplied evidence.

Release hosts need a published CLI containing the recovery fix. This change does not backfill v0.13.1 or relabel newer artifacts; an already-tagged invalid candidate remains a visible recovery blocker.

Validation includes 79 focused Swift tests, release and test builds, prompt and conversation checks, and manual-release, scheduled-release, and migration tests. Release recovery is established by local fixtures, not a newly observed public recovery.