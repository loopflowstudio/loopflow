# Capture Tasks demo — 2026-10-02

Jack Heart requested launching the LOO-368 demo. Built and opened Loopflow Dev
from this Task checkout with `uv run python scripts/loopflow-dev.py run`.
Swift build, bundled control CLI build, signing and LaunchServices open succeeded.

The configured launch is blocked before end-to-end capture: installed `lf skill
show capture-tasks` reports `skill not found`. The development app's control
configuration points at the machine-selected CLI gate, and its bundled CLI
forwards to that installed runtime. The design's earlier assumption that a
branch-built app alone supplies the new skill is insufficient.

Jack has not yet supplied visual feedback or accepted a button label. No capture
conversation or cross-repository filing was demonstrated, and no Flow cursor or
review gate was completed. Next: provide a configured runtime containing both
the new CLI launch syntax and capture-tasks, then exercise the walkthrough in
[capture-task.md](capture-task.md). The current UI is open for visual review.

## Screenshot and naming review — 2026-10-02

Jack Heart supplied a Desktop screenshot confirming the Create Task action opens
a shell whose CLI rejects `capture-tasks`, then requested comparison with Codex,
OpenCode, Notion, Linear and Asana before settling the button.

Local demo workaround: linked `~/.lf/skills/capture-tasks.md` to this checkout's
builtin skill. Installed lf 0.12.31 resolves it through `lf help capture-tasks`
and assembles `lf context --skill capture-tasks --json` from the main repository.
This supersedes the missing-skill blocker for this machine, but does not prove a
live Desktop conversation or repair release packaging. Remove the local link
when the builtin ships so it cannot shadow future releases. No runtime or shared
store was replaced. The button still needs a fresh click to verify provider launch.

Naming remains undecided. Agent proposal for Jack's review: a compact compose
entry labeled New conversation, with the initial prompt explaining that ideas
can become Tasks. The current large filled row looks selected at rest and claims
immediate Task creation although capture begins an open-ended conversation.
No label change is accepted or implemented.

## Accepted visual reference — 2026-10-02

After reviewing the comparison board, Jack Heart said, “I think Linear is the
right model here.” Use Linear's dedicated compose row beneath workspace identity
as the reference for the capture entry point: icon, short label and modest filled
treatment. In Loopflow, repository identity occupies that header position.
This supersedes the agent's proposed compact header-icon direction above.

Interpretation: retain a distinct creation row and refine its proportions and
emphasis against the Linear reference. Jack has not selected exact dimensions or
a final label; choosing Linear does not by itself approve “New issue,” “New Task,”
or the existing “Create Task.” The accepted exploratory capture behavior remains.
Reference board: /Users/jack/src/loopflow.product-wave/scratch/creation-board/index.html.
Official visual reference: https://linear.app/docs/custom-views.

## Naming accepted

Jack Heart accepted **New Session** and then explicitly requested keeping the
existing opening prompt. Updated the sidebar and Wave-menu labels, used the
compose icon, and reduced vertical padding from 10 to 6 points with a 5-point
corner radius. Those exact proportions are implementation choices for review.
The capture skill and launch prompt are unchanged.

Check: `scripts/test_desktop.sh --filter TaskCaptureTests` and `git diff --check` — passed; live appearance remains for review.

## Skill-picker prototype accepted

Jack Heart approved the final prototype appearance with “ok works” after
adjusting the skill label and chevron. The accepted reference is
`/Users/jack/src/loopflow.product-wave/scratch/session-picker/index.html`.

Keep the compact Linear-style split row: New Session launches, while the skill
name opens the searchable picker. The final prototype uses matching 13-point,
550-weight labels, a left-aligned skill name with 12-point horizontal padding,
a 7-point gap before a 12-point SVG chevron, and a -0.5-point vertical chevron
offset. Jack requested preserving the existing opening prompt.

This confirms prototype styling; the configurable picker is not implemented in
Desktop yet. The prototype uses illustrative skills, local browser persistence,
and simulated launches. Native skill discovery, selection persistence and real
launch behavior still need implementation and verification.

## Pursue launch recovery

Jack Heart requested launching pursue. Task restart checkpointed work as
`aa3506246d848ca6ecbf85c23f01f5210a944bef` and replaced feature with managed pursue
`12ff7516-5fc7-4cb6-a602-452897965dc6`, but detached worker startup timed out and
readback confirmed no active Task Session. Direct `lf --task LOO-368 --mode batch
pursue` then started Task-associated Flow `22608ecf-787b-4cbc-8e8d-4c322717e387`.
Its implement Session `session_b4b784b6ae0b4592adaaaad93342caf9` is live and confirmed
it is implementing the approved picker. The managed Flow remains idle; do not
resume it concurrently. This direct Flow retains pursue's publication and human
demo boundary. Managed/direct Flow reconciliation remains unresolved.

## Native picker review preparation — 2026-10-03 UTC

This review follows [the current design](capture-task.md) and
[the accepted picker reference](session-picker.html). No new feedback or native
acceptance from Jack Heart has been recorded in this review yet.

The current checkout's `swift build` passed, compiling the picker and launch
controls. Linking emitted two Ghostty ImGui symbol warnings. Both the installed
CLI and the app-bundled CLI returned capture-tasks in `lf list --json`, sourced
from the previously recorded local skill link. This proves configured discovery,
not shipped builtin packaging or an interactive provider launch. The attempted
`lf skill show capture-tasks` was rejected by the active human Flow's skill guard;
`lf list --json` is the successful discovery evidence.

Loopflow Dev was already running from this Task checkout. Its installed executable
differed from the local build, and the local build predated the latest picker
source before rebuilding. Restart is pending Jack's confirmation that existing
terminal drafts are saved; the normal dev launcher stops the running app. No
restart or Session completion has been performed by this review.

Next: install/open the rebuilt app once drafts are safe, then have Jack inspect
the compose row, search and keyboard selection, confirm selection alone does not
launch, and launch capture-tasks from an existing Task while checking both retained
layouts. Continue a real idea through ownership and useful Task text in the same
capture conversation. Record actual observations before marking the review ready.
Native appearance, provider/draft continuity, cross-repository filing and uncertain
write recovery remain unproven; no design change or approval is inferred.

Check: `swift build` passed with two linker warnings; configured `lf list --json`
and bundled-CLI discovery passed using the local skill link.

## Synced build reopened — 2026-10-04

Jack Heart requested `lf sync` and reopening the app, authorizing the pending
restart. Bare `lf sync` was ambiguous; `lf task sync` merged main. Its conflict
helper could not run within the human review Session's skill guard. The Product
memory conflict was resolved inline, retaining capture decisions and main's
Session working-set update; `lf task sync --continue` completed locally. The
stashed demo notes were restored using the recovery command supplied by lf.

`uv run python scripts/loopflow-dev.py run` rebuilt Swift and the bundled CLI,
signed the app and reopened Loopflow Dev from this checkout. This supersedes the
pending-restart status above. No new native feedback or approval is recorded;
Jack can now review the picker and capture launch in the reopened app.

Check: dev build/sign/open passed; `scripts/test_desktop.sh --filter TaskCaptureTests` passed all six tests; native review and capture-to-owner proof remain open.

## Demo approved — 2026-10-04

After the synced build was reopened, Jack Heart said, “ok, demo approved”.
This is Jack's approval of the presented demo. No design changes or additional
implementation requests accompanied it. The accepted contract remains
[capture-task.md](capture-task.md), with the visual reference in
[session-picker.html](session-picker.html).

Jack did not report individual scenario results. Approval does not independently
prove cross-repository filing, uncertain-write recovery, provider/draft continuity,
release packaging, or sustained-use KRs. The build and six focused passing tests
are recorded above; those evidence limits remain explicit.

Recommended next action: complete this human review and let the following
loop-decide interpret the approval and remaining evidence against the authored
Flow. This review does not choose a navigation edge or establish delivery.
