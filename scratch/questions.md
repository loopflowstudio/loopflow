# Open design questions

2026-10-01 — capture behavior is agreed; the Desktop button label remains open.

Jack Heart accepted the design on 2026-10-01: **Create Task** launches the
single **capture-tasks** skill from repository or Wave scope. Exploration adapts
to the idea; filing happens once intent is clear. The conversation remains at
its launch scope for additional Tasks across Waves and repositories. Existing Wave conversations share capture
behavior; owners operate captured work; `design` stays available.

Jack Heart subsequently authorized Product placement and launch on 2026-10-01.
Product's recommended `feature` Flow preserves kickoff and review boundaries.

## Kickoff assumptions (2026-10-01)

Made without Jack Heart's confirmation; each is reversible.

- With a Project selected, Create Task uses that Project's Wave. The accepted
  design names only overview, Wave and Task.
- When the selected Wave's name cannot be resolved, Create Task opens at
  repository scope instead of disabling itself.
- `wave/session`'s "Develop direction" section is replaced by a pointer to
  `capture-tasks`. This drops its instruction to write ideas under `scratch/`
  and its "once the user has agreed" filing condition in favour of capture's
  "file when intent is clear, honor existing authorization". The accepted
  design says Wave conversations share capture; it does not mention this
  section by name.
- The Wave menu action appears on Wave rows only. Task rows reach capture
  through the visible button.

## Review handoff (2026-10-01)

The [current design](capture-task.md#capture-launch-review-2026-10-01) preserves
Jack Heart's accepted scope. Source review clarified the explicit repository
workspace destination and added a cross-checkout pane-preservation check.
These are implementation clarifications; the assumptions above remain unconfirmed.
Jack Heart clarified that capture can produce many Tasks across Waves and
repositories, that initial Wave attribution can be wrong, and that Tasks are
the data model for intention. Jack Heart selected `capture-tasks` as the revised
skill name. These decisions supersede the singular skill name and any reading
of launch context as a filing boundary. Jack Heart subsequently reopened the
button label: New, New Session, and Design are candidates; Explore an idea is
an agent proposal. Create Task remains only a placeholder in the design.

Jack Heart clarified Task sizing: prefer larger, cohesive Tasks with obvious
behavioral promises and benefits. Premature decomposition can misrepresent how
work unfolds. Choose boundaries carefully, considering opportunities to pipeline
or parallelize, rather than imposing one Task per independently useful outcome.
This supersedes the earlier decomposition rule.

Jack Heart approved continuing with this revised design on 2026-10-01. The
button label remains open; no candidate was selected by that approval.

Next useful action: implement the builtin capture skill and shared Wave pointer,
then Desktop launch and production-control checks as one delivery unit. The real
demo must still establish useful Task text, no worker launch, multiple ideas
in the same capture Session, corrected attribution, and cross-repository filing.
Verify destination-scoped CLI reads, writes, and uncertain-write reconciliation;
partial success must not cause successful Tasks to be recreated. Review completion and Flow navigation remain outside
these design notes.
