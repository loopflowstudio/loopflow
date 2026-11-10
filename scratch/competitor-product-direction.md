# Report card and next-quarter product proposal

2026-11-07. Product direction with nine filed outcomes; no execution started by this filing. Based on four code investigations and a focused read of Loopflow Desktop in LOO-593. Competitor workflows are implemented; public evidence of user preference is useful but anecdotal. No comparative live-use or reliability benchmark was performed.

## Report card

Grades express architectural fit and implementation completeness visible in this investigation, not measured uptime or satisfaction. A = strong fit/evidence; B = substantial capability with meaningful limitations; — = not assessed sufficiently. Plus/minus distinctions are judgments. Loopflow's cross-device grade is omitted: the inspected 593 protocol is local-only and the separate mobile effort was not audited.

| Product | Factory/workflow model | Foundation for custom automation | Cross-device implementation | Maturity judgment |
| --- | --- | --- | --- | --- |
| Loopflow | A | A− | — | Most explicit durable delivery model; significant platform cutover still in progress. Fit does not establish reliability. |
| T3 Code | B+ | A− | A− | Strong command/recovery architecture and shared client system; rapid change and a large integration surface. |
| Orca | B | B+ | A− | Broadest operator-workspace candidate in this sample; substantial ownership/recovery machinery and corresponding complexity. |
| Paradev | B | B+ | B+ | Established worktree/workspace product; current surface semantics and documentation disagree. Mobile platform coverage is narrower. |
| Paseo | B | A− | A− | Attractive reusable daemon/SDK and portable-client substrate. Some lifecycle features, including terminal survival through daemon restart, are recent. |

For a factory built around repeatable Task-to-PR delivery, Loopflow's model fits most directly. Among the four external products, T3 is the first candidate to trial: durable command intent, provider handoffs, orchestration and cross-device supervision reinforce each other. T3 has the strongest overall upside in this comparison as a broad agent product. Loopflow's specialized upside is repeatable delivery with persistent intent and evidence; realizing it requires an equally legible daily experience. Orca leads on product breadth; this investigation does not establish a stability winner. Paseo is especially attractive as a substrate to extend.

Current releases checked: [T3 v0.1.7](https://github.com/pingdotgg/t3code/releases/tag/v0.1.7), [Paradev desktop-v2.0.5](https://github.com/ParadevAI/Paradev/releases/tag/desktop-v2.0.5), [Orca v1.4.253](https://github.com/stablyai/orca/releases/tag/v1.4.253), [Paseo v0.15.3](https://github.com/getpaseo/paseo/releases/tag/v0.15.3). Version numbers and release frequency are not maturity scores.

### Expanded product report card

A = a clear strength in the inspected implementation; B = useful capability with limitations; C = a material gap in the inspected surface; — = insufficient evidence. These are comparative judgments from source inspection, not measured usability scores. A category can grade well without being unique. Loopflow grades distinguish its durable engine from the Desktop experience inspected in LOO-593; mobile remains ungraded until LOO-718 is audited.

| Product category | Loopflow | T3 Code | Paradev | Orca | Paseo |
| --- | --- | --- | --- | --- | --- |
| Durable intent, planning and Task-to-delivery model | A | B | B | B | B |
| Reusable delivery workflows | A | B | B | B | B |
| Making those workflows approachable in the workspace | C | B | B | B | B |
| Supervising several pieces of work | B | A | B | A | B |
| Returning to meaningful changes and next actions | C | A | B | B | B |
| Selection and artifact references in a reply | C | A | B | B | — |
| Integrated diff/review feedback | B | A | A | A | B |
| Independent workspace surfaces | A | B | A | A | B |
| Native terminal as a first-class surface | A | B | A | A | B |
| Mobile supervision and continuation | — | A | B | A | A |
| Headless host and programmable control | A | A | A | A | A |

The equal grades in programmable control are intentional: all five have substantial automation capability. Loopflow's differentiation is the semantics of repeatable delivery, not exclusive possession of an API. The workspace row also corrects an earlier overstatement: independently useful panes are already a Loopflow strength.

### Expanded architecture report card

Architecture grades assess how explicit the relevant contract is and the concrete implementation inspected, not whether the entire product has proved the guarantee under load.

| Architecture category | Loopflow | T3 Code | Paradev | Orca | Paseo |
| --- | --- | --- | --- | --- | --- |
| Durable app identity separate from provider/process identity | A | A | A | A | A |
| Execution ownership and attachment authority | A | B | B | A | B |
| Structured-state recovery after disconnect | B | A | A | A | A |
| Terminal replay and rendering recovery | B | A | A | A | A |
| Durable mutation intent / retry accounting | B | A | C | — | A* |
| Portable shared client/runtime boundary | B | A | A | A | A |
| Explicit provider transition semantics | B | A | B | A | B |
| Remote admission and host access | — | A | B | A | A |
| Observed production reliability under a common benchmark | — | — | — | — | — |

*Paseo's durable receipt strength is specifically resource creation, not a claim that every command is exactly-once. Loopflow's honest unknown-outcome handling is valuable, but does not equal durable transparent retry. Paradev's in-memory command dedupe does not survive host restart. Orca mutation replay was not assessed sufficiently to grade. Loopflow's local full-reading recovery is simpler than incremental replay and is not inherently inferior; the B reflects the cutover/recovery experience still being completed.

Terminal/chat interoperability deserves a separate descriptive row because a single grade would hide different guarantees:

| Product | What a surface switch preserves |
| --- | --- |
| Loopflow | Retained Session and explicit native-terminal attachment authority; a universal structured-chat/native-TUI toggle was not established. |
| T3 Code | Integrated shell and structured chat coexist; best-effort native import is separate from a live surface toggle. |
| Paradev | Current code forks the conversation in either direction and preserves the source. |
| Orca | Terminal-backed chat can use the same PTY; CLI → structured can resume after writer checks; structured → CLI forks. |
| Paseo | Manual native-conversation import/resume/reload round trip; no atomic live ownership transfer established. |

Evidence: [T3 investigation](competitor-t3code.md), [Paradev investigation](competitor-paradev.md), [Orca investigation](competitor-orca.md), [Paseo investigation](competitor-paseo.md), and [Loopflow protocol comparison](competitor-protocol-notes.md). Onboarding, accessibility, latency, stability and total cost still need a shared hands-on trial; a guessed letter would obscure that missing evidence.

## The quarter's outcome

A person can start several pieces of work, leave them running, return on desktop or phone, understand what changed, make precise decisions, and move verified changes toward delivery without reconstructing context by hand.

Loopflow already has independent workspace panes, retained Sessions, Tasks, Graphs, files/diffs, a navigator and availability readings. Connect these existing pieces into a short operating loop. The distinctive opportunity is to make intent, evidence, decisions and delivery useful together at the moment someone acts.

## 1. Supervision and returning to work

After time away, someone should immediately see which work is available to discuss, what changed, and what useful action they can take.

Borrow T3's cross-environment inbox and thread organization. Build on Loopflow's existing Available/Busy projection, Task state, Graph position and PR evidence. Show the last meaningful change, relevant question/review and destination. Keep unread/read markers separate from execution state. Offer one command to visit the next selected kind of work. A reminder or snooze changes presentation, not Task completion, Session availability or Graph execution. Preserve keyboard focus as updates arrive.

Loopflow's distinctive version attaches each reason to actual Work: a review is available, checks failed, a result arrived, or a decision remains open. Opening it lands on the relevant artifact beside its ongoing Session. Scope can span repositories without losing Task/Wave context.

Measure time from reopening to identifying the next useful action, repeated opening of unchanged work, and unnecessary notifications. Establish baselines before targets.

## 2. Artifact review and precise feedback

Someone should be able to point to the thing that needs changing and continue the conversation without manually copying and explaining its location.

Borrow T3's selection-to-context chips and the diff-to-agent patterns in Paradev and Orca. Start with terminal, Markdown or file selections becoming visible references in an editable reply draft. Collect line/hunk comments and submit them together. Preserve file, revision and excerpt so later edits do not silently retarget feedback. Show the receiving Session before submission; selection alone never sends.

Add comparison against the last reviewed version, not only the branch base. This is a proposed extension of the demonstrated workflow, not a claim that every competitor implements it.

Loopflow's distinctive version links feedback to the Task and reviewed evidence. Design decisions, implementation diffs and verification results remain in one work history. When the agent returns, show what addressed the feedback and what remains unresolved. Make this convenient review rather than mandatory administration.

Measure manual copy/paste steps, time to send an unambiguous correction, review repetitions, and time from agent hand-back to delivery decision.

## 3. Continuous work across desktop and phone

Leave the desk during a run, inspect meaningful results elsewhere, respond, and return to the same work without identity or draft surprises.

Prioritize one complete mobile loop: find work, read the latest result, inspect its diff/document, answer or steer, and understand whether the response reached its destination. Preserve Work/Session identity through reconnection and installation changes. Show host identity and connection/freshness state; retain drafts. Connection loss must not produce a new provider launch or duplicate mutation.

The first mobile experience need not reproduce every desktop pane. Pairing, remote admission, lifecycle and delivery semantics are foundational work. Preserve explicit terminal input/geometry ownership: a passive phone view should not seize Desktop's terminal.

Loopflow's distinctive version lets phone actions operate on the ongoing Task and workflow, with the same intent and delivery context as Desktop. Measure complete device-continuation journeys, lost drafts, duplicate actions, and recovery through sleep/network change/host update.

## 4. Make repeatable delivery approachable

A workflow that produced a good result once should be easy to run again, inspect and improve.

Expose a few useful existing Skill/Graph workflows through the normal workspace: fix a bug, implement a feature, address review feedback, maintain a repository. Show current step, result and next action in plain language. Reuse Tasks, Graphs and ownership instead of introducing a second UI automation system. Keep scope, checkout, verification and delivery destination easy to find.

Adapt a successful workflow into reusable instructions with an editable preview; preserve what past runs actually executed. Start from explicit intent rather than inferring permanent rules from every correction.

Loopflow's distinctive version improves delivery workflows with retained intent and evidence, not only prompts. Wave memory can inform later work; actual outcomes reveal which changes helped. Measure time to first verified PR, repeated workflow success, interventions per delivered change, and review/verification failures. Agent count and token volume are costs, not the success metric.

## UX details to take first

| Priority | Interaction | Existing Loopflow foothold | First useful slice |
| --- | --- | --- | --- |
| 1 | Select text → Reference in reply | Ghostty selection reads; current Copy/Paste/Clear menu | Reference action and editable draft with visible destination and source |
| 2 | Comment on diff → send one coherent review | Existing file/diff/PR panes | Several anchored comments sent together to the ongoing Task Session |
| 3 | Return to what changed | Available/Busy, Task/Graph/PR projections and navigator | Meaningful change preview and Next available work without redefining execution state |
| 4 | One keyboard path to a Session/action | WorkPalette and DesktopActions | Direct Session destinations and frequent contextual actions in the existing navigator |
| 5 | Keep work legible during reconnect | 593 native connection status and stale-screen recovery | Retain reading/copy/draft; avoid reordering and focus theft |
| 6 | Dictate into an editable draft | Later input enhancement | Try dictation after references/review; public evidence is weaker for full conversational voice |

The first two are the strongest immediate UX investments. The inbox/return experience follows. Full voice mode, another terminal renderer, more provider logos or a new generic orchestration abstraction have weaker support as the quarter's leading bet.

## A 12-week sequence

- Weeks 1–4: selection references, anchored review drafts, Session/action navigation and return-to-work summaries. Establish first-task/review baselines. Finish relevant 593 recovery work already underway.
- Weeks 5–8: one complete desktop → phone → desktop supervision journey with pairing, freshness and honest mutation outcomes. Continue the review loop across devices.
- Weeks 9–12: package a few proven Graph workflows around verified delivery, expose past outcomes and remove setup friction. Expand after complete journeys work reliably.

Onboarding, accessibility, keyboard behavior and reliability belong in every slice. The first successful Task should demonstrate the model before the interface asks someone to learn its vocabulary.

## Evidence and open questions

Detailed sources are in the competitor research report. Local UI inspection covered TaskFilesView, TaskPullRequestView, WorkPalette, SessionsView, ChatReference and Ghostty selection/context-menu code in LOO-593. This establishes nearby implementation affordances, not an exhaustive inventory of every worktree.

1. Is the initial beneficiary a single developer supervising several tasks, or a team coordinating delivery? This sequence assumes the former as the first complete journey.
2. Which mobile continuation paths are already proved in LOO-718? Audit them before allocating duplicate implementation.
3. What are baseline review and recovery failure rates? Set acceptance targets from those rather than invented metrics.

## Filed outcomes

Seven new Tasks and two refined existing Tasks in the Loopflow repository. All remain unstarted. Product owns the new outcomes and LOO-434; LOO-403 retains its existing Infrastructure membership. Existing Task descriptions and open design choices were retained.

| Task | Filing | Wave |
| --- | --- | --- |
| [Reference terminal and document selections in an editable Session reply · LOO-793](loopflow://loopflow/task/LOO-793) | Created | product |
| [Collect anchored diff comments into one editable review draft · LOO-794](loopflow://loopflow/task/LOO-794) | Created | product |
| [Show meaningful changes since the last visit and open their useful destination · LOO-795](loopflow://loopflow/task/LOO-795) | Created | product |
| [Make the Mac app's shortcuts findable and put commands in the palette · LOO-403](loopflow://loopflow/task/LOO-403) | Refined | infrastructure |
| [Keep reading, copying, drafts and focus usable through Desktop reconnect · LOO-796](loopflow://loopflow/task/LOO-796) | Created | product |
| [Make reviewing and correcting agent work effortless across revisions · LOO-797](loopflow://loopflow/task/LOO-797) | Created | product |
| [Make returning to several running Tasks feel easy · LOO-798](loopflow://loopflow/task/LOO-798) | Created | product |
| [Continue a Task from Desktop to phone and back with its context intact · LOO-799](loopflow://loopflow/task/LOO-799) | Created | product |
| [Make repeatable delivery approachable from the Task workspace · LOO-434](loopflow://loopflow/task/LOO-434) | Refined | product |

LOO-797 consumes selection/review slices LOO-793/794 and owns continuity across reviewed revisions, preserving LOO-733’s open architecture decisions. LOO-798 composes LOO-795’s change previews and LOO-403’s navigation. LOO-799 extends the complete journey on top of LOO-718/593 rather than rebuilding their mobile/server foundation. LOO-796 covers residual reconnect interaction gaps and composed proof.
