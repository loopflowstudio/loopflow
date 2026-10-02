# LOO-369 prototype review and recovery · 2026-10-02

Jack Heart approved the [integrated prototype](task-history-inline-prototype.html)
in the live October 2 conversation and explicitly requested its Desktop
implementation. The earlier pending-acceptance boundary is superseded. This pass
implements only; the caller owns queue and delivery. The recovery observations
below remain historical evidence, not fresh Flow status or recovery authority.

## Reviewable interaction

The latest completed manual contribution, Session
`session_c5b909e562bf43efb49eda96a3179369`, records Jack Heart's requested revision:
one checkbox-and-label pill, inline numeric editing, and zero labeled All Tasks.
Its completion reports implementation and headless checks, not Jack's acceptance.
The earlier adjacent range-menu research is an alternative explored before this
revision, not the current implementation direction.

Open the standalone HTML locally. It uses fictional Tasks and no account data.

| Action | Expected visible result |
| --- | --- |
| Initial state | Unchecked Completed; 2 current Tasks |
| Click Completed | Checked 7 Days; 3 Tasks |
| Click 7 Days | Number selected in place, Days suffix retained |
| Enter 30 and press Enter | 30 Days; 4 Tasks |
| Enter 0 and click away | All Tasks; 6 Tasks, including unknown-date completion |
| Click All Tasks | Editable 0 Days in the same pill |
| Change the draft and press Escape | Previous applied range and count retained |
| Enter a negative, fractional, empty, or overflowing value | Feedback; previous applied range retained |
| Uncheck, then check | Current-only rows, then the remembered range |

Canceled duplicates remain excluded throughout. These are prototype counts,
not Growth's live inventory. The existing production proof separately retains
canceled work with unresolved execution and access to active Sessions.

The approved interaction is now the Desktop implementation target. The earlier
menu proposal is superseded. All Tasks means current work plus successful
completion history, not canceled inventory. Native layout must preserve number
and suffix placement across editing; long integers must not widen the control.

## Evidence and remaining proof

Read the completed contribution's native record and its existing browser-check
script at `/tmp/task-history-check.mjs`. The recorded check covers counts,
duplicate exclusions, focus/selection, Enter/blur, Escape, invalid input,
remembered range, checkbox clicks during editing, keyboard activation and the
checkbox's accessible name. This manual pass did not rerun it or claim native
Desktop acceptance. Prototype SHA-256 remains
`1fb7c3ef32ca58aa42f01880e3ef8efb0659ee0989e13558d8bac783e2936688`.

The earlier production gate and configured app build passed as recorded in
`docs/reviews/task-history.md`. The accepted Swift revision now has seven passing
focused tests and SwiftPM app compilation, including native control interactions
and rasterized glyph-position comparison. The main design records the exact
command/result; full gate belongs to the caller. Growth refresh and retained workspace /
native Session reopening still belong to the authored demo review.

## Preserved recovery boundary

Fresh `lf task status LOO-369 --json` reports blocked execution at `loop-decide`,
index 6, iteration 0, invocation `f8730e54-25b0-411d-aa13-4a478d5138ff`, failed
event 4293. The worker field is null. The latest earlier manual contributions
are completed; this pass is the current manual contribution. `lf ps --json`
also shows the existing unblock conversation's waiting Execs; their presence
does not authorize interrupting them or launching a competing worker.

Fresh Session inventory retains
`ask_once_5bba44fcd30bd861ec7b3bb093d627151a93ef75dd5455a27e7eec79efd0946c`
as ready, with the tested schema-repair summary. Ready is not completion.
Its captured runtime still points at the installed executable. The branch repair
cannot change that running caller merely by existing on disk.

An authorized recovery must first use a runtime containing the repaired schema,
then preserve the exact failed occurrence and existing review completion contract.
Do not retry release 0.12.29 unchanged. Provider acceptance and decoding of the
successful decision completion remain unproven. This contribution changes no
installation, Flow cursor, Session completion, PR, or Task state.

Check: read-only Task/Session/native-history inspection and prototype hash — consistent with the retained review boundary; `git diff --check` — passed; no new execution or visual acceptance claimed.
