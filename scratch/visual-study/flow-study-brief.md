# Task 1: preview its Flow before starting

Build three interactive visual alternatives for the same unstarted Task. Jack
explicitly asked for Claude/Opus mockups. This is a bounded contribution: edit
ONLY new `scratch/visual-study/flow-options.js` and `flow-options.css`.
Parent owns integration, comparison page, references, validation and presentation.
Do not commit, launch providers, write Linear, alter native code or other files.

## Contract

Parent will load your CSS and JS in mockups.html before its final render().
Expose `window.renderTaskFlow(task)` returning HTML. Parent calls it between the
Task heading and Description only when URL `flowstudy=a|b|c` exists. It should also
include collapsed Comments disclosure (after Flow is fine). Existing global
`render()` rerenders the page. Use document-level listeners with your own
`data-flow-*` attributes, not existing data-act. Task object has id and title.
Keep state by Task and option; preserve selection across rerender. Your script
does nothing unless flowstudy is set. Prefix CSS .flow-study etc. so current
page isn't changed. Read mockups.html/current-plan.js for frame/font/color context.

## Three genuinely distinct treatments

A — Compact route. Inspired by Loopflow's Feb 3 FlowProgressPills: a modest
horizontal sequence with connectors, beside/under a Flow name and Start. Step
selection reveals one short detail beneath. Change Flow in a small disclosure.
Most space remains for the Task description. This is the likely calm default.

B — Step outline. Inspired by Zapier's outline: vertical connected steps with
one-line purposes; expand a step for detail. Compact launch controls alongside
the outline or above it. More explanation before committing to execution.

C — Flow map. Inspired by GitHub Actions connected job cards: a contained
horizontal node diagram, with the nested Code flow visibly grouped. Selecting
a node opens a compact adjacent/below inspector. A tucked-away Change Flow
action can open a panel. This is a preview, not a drag-and-drop graph editor.

All three: clear Start button; no progress, checks, running glow or elapsed time
because nothing has started. Show chosen Flow, execution order, and meaningful
human boundary if present. Comments collapsed by default; no invented comment
count or records. Expanded comments may say 'No comments in this study' and
have a disabled composer labelled as a prototype if necessary; simpler is fine.
Start only shows a small clearly labelled prototype confirmation naming Task and
Flow; NEVER launches execution or alters sidebar started membership. Can dismiss.

Flow picker uses actual local definitions (default Build is illustrative, NOT a
claim about current Wave configuration):
- Build: Kickoff → Code (Implement → Compress) → Review slice → Demo.
- Task design: Kickoff → Review design (explicit human approval).
- Code: Implement → Compress.
Switching Flow updates diagram, step details and Start confirmation. Human marker
only for task-design Review design; do not invent approval gates on Build.

## Accepted visual language

Warm paper, burgundy, quiet olive/ochre accents from existing mockup. Sans-serif
Lato Task title. Keep existing Wave / linked issue breadcrumb, one title, repo
header/sidebar continuity, bottom search. No Local work evidence, Open header,
ELSEWHERE badges, Linear snapshot label or internal attribution diagnostics.
Parent labels fixture limits in study chrome, outside product UI. Task title and
Description remain exact captured Linear text. Use accessible real buttons,
visible focus, aria-expanded, no hover-only essential interactions. Fit the
available main pane at 1100px viewport with sidebar; wrap or scroll the diagram
within its own region if needed. Do not add a large empty hero or generic cards.

## References inspected by parent

- Old Loopflow pills: https://github.com/loopflowstudio/loopflow/commit/e6bb6756de75c579f00ccb6b4e69ddc97564b44d
- February native screenshot: flow-references/loopflow-february.png
- GitHub graph: https://docs.github.com/en/actions/how-tos/monitor-workflows/use-the-visualization-graph
  local screenshot flow-references/github-actions.png
- Zapier outline: https://help.zapier.com/hc/en-us/articles/8496181725453-Learn-key-concepts-in-Zap-workflows
  Step selection opens details: https://help.zapier.com/hc/en-us/articles/16722578092429-Use-the-editor-to-build-and-view-your-Zap-workflows

Finish by reporting exact exported hook and interactions implemented. Parent
will inspect and test browser behavior. No broad suite or new tests necessary.
