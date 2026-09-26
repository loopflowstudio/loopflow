# Scratch feedback handoff

Accepted direction, 2026-09-25: scratch holds notes from demos, reviews,
research and other work. They should remain generally useful regardless of
where they were produced or which skill runs next. Keep loop-decide separate;
review-slice and demo should not also own the navigation judgment.

Convention: topical Markdown notes with context/date, observations and human
feedback, agreed changes, unresolved questions, and the next useful action/proof.
Link the current design and supporting evidence. Preserve useful observations
and mark superseded conclusions. No mandatory handoff filename or machine
schema. Existing delivery cleanup policy is outside this change.

Ready summaries carry exact note paths and a short takeaway. Loop-decide starts
there, reads the current design, reconciles other relevant scratch evidence,
and carries the same paths into its chosen direction. Recommendations are input
to its decision, not navigation authority. Missing paths do not block discovery:
look at note topics/content. Missing evidence remains an explicit gap.

The existing recursive Markdown loader already includes scratch contents in
fresh Runs. The focused behavioral proof now writes nested search feedback and
design notes, completes the review with their paths, and checks actual prompts
for both the next loop-decide and the following implement step. Both must get
the notes' full contents; the files must remain available. This distinguishes
a useful handoff from merely mentioning a nonexistent path. Provider effects
are simulated, and this does not measure live model judgment quality.

Verification: 16 focused tests passed (builtin contracts, committed/untracked
scratch prompt assembly, and nested review → decision → implementation handoff).
The proof checks the actual prepared prompts contain both the referenced note
and linked design, with neither another copy nor a producer-specific input
mechanism. Provider execution is simulated. Skill metadata, guide YAML,
formatting and diff whitespace checks passed.

Review cases: standalone demo leaves useful notes; arbitrary topic/nested paths
reach both consumers; older unresolved findings are reconciled; newer proposals
cannot silently replace accepted direction; omitted paths trigger discovery;
missing substantive evidence remains a gap. These prompt instructions guide
judgment, while the test proves transport only. No new control file, filename
parser, retention policy or navigation endpoint was introduced.
Final all-target Clippy passed with warnings denied.
