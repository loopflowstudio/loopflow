# Terminal-host keyboard trials · 2026-10-07

Jack Heart authorized opening separate windows for specific UX tests, superseding
the earlier prohibition for these newly created trial surfaces only. Existing
windows/conversations remain untouched. Real-provider commands are staged without
Enter; Jack starts them. No automated login or credential inspection is authorized.

The review Session created cmux window 8DBB1730-3E0D-451F-B06C-2AA06F3FDF6A.
Its guide workspace is 9FD33F4A-3A74-4357-8102-8B640A94317C.
An unrenamed test workspace workspace:1000000017 / surface:1000000026 holds:

`lf -m claude : 'UX trial only. Do not use tools or edit files. Ask Jack one short question about a color and wait for his answer. After he answers, repeat the color and wait for another message.'`

Jack reported: “ok that one seemed to work fine”. This is overall interactive
trial feedback, not independent confirmation of each title, status, notification,
color or focus observation. It does not erase the earlier Task-bound large-prompt
failure: the launch path and prompt differ.

Next proposed trial: a taskless two-skill Flow, ux-first then ux-second, each
asking the real provider to announce itself, sleep twenty seconds, and finish.
Prompts live in a disposable directory outside the implementation checkout.
No injected host status, notifications or title signals. Jack starts the staged
command and observes current-step propagation, early completion and final outcome
while looking at another workspace.

Jack supplied a screenshot after asking Claude to use its special Questions UI.
[The screenshot](terminal-host-keyboard-question.png) shows an active color
multiple-choice question and cmux sidebar text “Agent is asking a question”,
“Needs input”, and “Running” concurrently. The sidebar/tab name is “Terminal host
trial in herdr...” rather than “Loopflow operating guide”. This establishes visible
question attention for this real interactive launch; desktop banner delivery,
attention clearing after answer, and Flow-step propagation remain unobserved.
Running may denote a live process rather than busy work; do not classify that
coexistence as a defect without testing the transition after Jack answers.

## Priority correction and second screenshot

Jack Heart: “i think headless flwos arent as improtant”. Prioritize the interactive
Session experience: launch, recognizable identity/title, actionable questions,
notification/attention clearing, continued input and native resume. Retain the
headless Flow observations as secondary evidence; do not turn full inner-step
propagation into a prerequisite for useful interactive adoption. This changes
priority without deleting the original Flow trial or claiming it complete.

[Jack's second screenshot](terminal-host-keyboard-flow.png) shows the previous
interactive workspace displaying “Blue.” and “Idle”, with no Needs input label.
That provides visible post-answer attention-clearing evidence. The new workspace
shows `lf -b -m claude run ux-flow` and Running while the terminal prints
`[1/2] ux-first` and its twenty-second sleep. The current step is visible in
terminal output but absent from the sidebar at that instant. No second-step,
final-state or notification-delivery result is established by this screenshot.

## Interactive Flow expectation

Jack Heart then tried `lf -m claude run ux-flow` and
`lf -m claude run ux-flow -i`, reporting “i dont seem to be able to run an
interactive flow?” His third screenshot shows the same streamed step output
for both commands, rather than the native Claude conversation interface.
Source inspection explains this: `Cli::step_args` always supplies `--batch`,
and the Flow process uses it when launching each skill. The top-level interactive
flag does not make those steps interactive. This is a Loopflow execution/UX
bug, not evidence of a cmux rendering failure. Jack identified it as a bug and
requested its repair in this branch plus a demo with the fixed binary. The repair
preserves explicit interactive/headless flags and the inherited terminal through
ordinary child skill commands and Task Flow launches. Successful native exit
advances; interruption or failure stops. Direct skills use the same mode rules.

History: `5bcc40fdff810c39885a31b3fbfd6557f49fde93` (PR #1283, September 25,
2026, 14:45 PDT) replaced forwarding the caller's launch mode with
`launch.batch = true; launch.interactive = false`. September 30's #1296 moved
the behavior into `step_args`; October 6's #1439 retained it. Initial attribution
to the later rewrite was corrected after tracing the predecessor.

The new PTY regression covers explicit `-i` before/after the Flow name, default
terminal launch, input reaching both successive native steps, and interrupted
first-step exit preventing the second launch. Provider substitutes are synthetic;
the signed-in cmux demo remains Jack's check.

Check: 2026-10-07 — inspected Flow skill launch and `Cli::step_args`; prose-only update, no provider launched or tests required.

## Fixed-binary cmux demo

Repair checkpoint: `56e207565`. The staged demo lives at
`/tmp/lf-host-ux-fixed.miw1fm1j`; `./try-flow` selects its private LF_HOME and
branch binary. Dedicated cmux workspace `workspace:1000000019`, surface
`surface:1000000036`, in the previously authorized trial window. Jack starts it.
The frozen binary at `bin/lf` has SHA-256
`d0e670b70b82fd3454c130a245d5cdcc4eb6e92f04ea6f0c5cda8224d8b5772d`.
The wrapper initially pointed to target/debug/lf while the build was finishing;
this hash identifies the frozen artifact, not independently the already-started
processes in Jack's screenshot.

[Jack's screenshot](terminal-host-keyboard-interactive-flow.png) shows native
Claude on ux-second, asking for an animal with a live input composer. Its tab
reads “Flow UX trial second step”; the sidebar shows that step's question and
Idle. This establishes real second-step native launch and visible step-specific
title/content, not final Flow completion or blanket acceptance. Plain-text
question Idle differs from the earlier structured question's Needs input.
Remaining check: Jack answers the animal question, types `/exit`, and observes
return to the shell without an error or stale Running state.

Jack's response to the native second-step screenshot: “i guess that works as
wlel as we could ope?” This is tentative acceptance of the shown interaction.
Native launch and step handoff pass the demonstrated baseline. The injected
prompt occupies most of the visible screen, and the prose question reads Idle;
these remain UX observations, not authorization for additional repairs. Final
shell return was not supplied, and this feedback grants no publication or landing.

Check: `cargo test -p loopflow --test flow_tests -- --test-threads=4` PASS 28/28; `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo build -p loopflow --bin lf`, `git diff --check` PASS. Review preserved the existing headless structured-answer correction path because `session resume ID MESSAGE` requires batch mode.

## herdr native trial prepared (2026-10-07)

Jack Heart requested continuing LOO-421 testing. Opened a separate Ghostty
instance with herdr 0.9.3 at the pinned revision, using private configuration,
state and socket under `/tmp/lf-herdr-ux-xast2ret`. Server PID 44007;
workspace `w2`, pane `w2:p1`. `./try-flow` is staged without Enter. It uses a
copy of the same fixed binary and two-step skills from the cmux demo, with a
new LF_HOME. Jack starts the real provider. No agent has been started by this
preparation.

The host receives an explicit environment without cmux hooks or inherited
Loopflow execution markers. Its shell starts without rc files. Normal HOME
is retained for Jack's manually initiated native provider; this is not the
credential-free automated fixture sandbox. Initial host snapshot and target
IDs are saved beside the socket. No titles or agent statuses were injected.

Jack started the trial. A subsequent socket read found native Claude on
`ux-second`, with `agent: claude`, `agent_status: working`, terminal title
`Claude Code`, and the correct fixture cwd. This establishes recognition through
lf and arrival at the second native step. Receipts beside the socket:
`session-snapshot-beep.json` and `pane-read-beep.json`.

Jack clarified that beeping occurs when the agent responds, then said “it seems
liek herdr is working as designed”. Record response-time sound as expected in
Jack's assessment, not a continuous-beeping defect. The precise sound source
was not established. Jack also raised a deliberate Continue action for Flows:
using Ctrl-C to move forward feels awkward. Next-step versus interrupt/stop
semantics remain an open design question; no new command or shortcut is accepted.

Jack subsequently replied “yeah seems like it works” to the final-exit check,
confirming the herdr interactive Flow trial and shell return. Jack then tried
the background-question check: ask a new interactive lf conversation to wait ten
seconds and use its Questions UI, switch herdr workspaces, observe attention,
return and answer, and check clearing. Jack replied “yeah seems to work”. Record
that scenario as a participant-reported pass; no separate screenshot or API
capture establishes the exact labels or timing.

Jack then reported “seems fine” after the resize/input scenario: type an unsent
sentence, resize smaller and larger, scroll up and back, then send it. Record
draft preservation and successful single submission as a participant-reported
pass, without instrumented input or rendering measurements.

Remaining native herdr check: resume. The prior cmux
second-step conversation remained open at the preparation read, so its final
exit is still unproven. These scenario confirmations do not establish the
signed-in Task checkout/publication path or authorize landing.

## Delivery decision · 2026-10-07

Jack Heart requested advancing this branch to land the fix after the interactive
trials. This supersedes the earlier local-only stopping boundary. Landing this PR
keeps LOO-421 open for remaining evidence and the unresolved Continue/Stop UX.
Native resume was discussed but no resume command was launched or independently
confirmed; Jack's later general feedback does not close that scenario.
