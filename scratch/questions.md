# Product decisions still open

The [plan](compare-cmux-s-command-line.md) owns implementation; [findings](findings.md)
own evidence. Jack Heart has already selected one shared Work tree, subtree
delegation, one repository window across machines, macOS-only view control and
one PR. Those choices are settled. Recorded location, local inspection/arrangement and
exact-target passive read transport are local; peer composition and native I/O
remain unfinished.

## Q1 — Selected: local operations and custom Git-ref Task sync (October 8)

Jack Heart selected execution-Machine-local operations and portable planning
exchange: LOO-406 owns the writer/Linear, LOO-412 Git-ref merge/scope. Workflow,
Processes and cleanup stay local; imported completion changes planning only.
Disconnect retains local work and pending sync. Q1 is resolved; composition proof remains.

## Q2 — Delegation inheritance

Jack accepted that started Tasks stay on their Machine for later runs/resumes.
Delegation applies only to open, unstarted and future Tasks. Nearest explicit
ancestor with narrower overrides remains proposed. Historical assignment
provenance stays unknown where copied and explicit values cannot be distinguished.

## Q3 — Selected: combine with the existing draft (October 8)

Jack Heart selected combining agent input with an unsent draft: “robably combineing
is right.” Use ordinary literal insertion at the current cursor; preserve existing
text. Enter stays separate and submits the combined composer contents. No silent
clear/replacement, draft parking or occupied-composer refusal is required. Exact
surface targeting is implemented; native text/key fidelity proof remains.

## Reversible visibility choice — October 8

Hide/restore address an existing retained view through the selected plan/window,
Machine/checkout and pane/content occurrence. Restore removes collapse without
selecting Work, focusing a window, changing zoom or acquiring a client. This
implementation choice leaves Q2 open. View occurrence tokens grant no
terminal-surface or composer-input authority.

## Reversible arrangement choices — October 8

An empty split, move and resize leave pane selection and zoom unchanged. Move
relocates a retained leaf after another exact pane (right for vertical, below for
horizontal), within one Machine/checkout; it never reassigns Work. Resize names
two exact panes and the target side's share of their separating divider. Zoom
changes visibility only; focus explicitly selects/reveals the exact pane in its
workspace, without switching Work or foregrounding the window. No view operation
authorizes native input. Q2 remains open.

## Reversible companion choices — October 8

CLI companions preserve focus, zoom and Work selection. An empty exact target is
filled; occupied content is never replaced. Files/Flow-log reuse and unhide the
named Task's pane only in its recorded/prepared checkout. Shell always allocates
and remains local-only until remote opening is composed. This does not decide Q2 or grant native-input authority. The verified lf3 artifact now supplies fixed-buffer extraction; native pane acceptance remains. Full-buffer reads followed by truncation are not a fallback.

## Reversible bounded-read choice — October 8

Extraction returns a prefix in terminal order, ending at a complete UTF-8 scalar,
with explicit truncation. Empty selection is successful empty output, not an
unavailable surface. Current selection includes a selected shell command block.
This supplies no input authority or semantic completion inference.

The read request uses a 64 KiB default and a 1 MiB maximum; the reply echoes its
exact target and distinguishes missing/nonterminal surfaces from empty
text. `hidden` describes pane collapse or zoom within its retained workspace,
not compositor visibility. The composed lf3 reader uses viewport/full-screen
selection bounds without changing the terminal's selection; no unbounded fallback.

## Command-map review — October 8

Open choices:
- Wave plan/report spelling is reversible: `wave show [WAVE]` retains the existing
  snapshot and cross-Wave `--all`/exact `--task` reads. `wave workflow set/source NAME [WAVE]`
  and `wave edit-plan [WAVE]` use current selection; `--project ID` addresses history.
  Project identity and Workflow capture stay with their existing owners.
- Skill/Flow name collisions. Flow preview presentation below is reversible.
- Final home for standalone prompt-budget reporting. Keep `--steers-after` and its existing filtering.
- Remembered repository names: Jack is unsure; draft uses root/name or explicit path.
- Discord is deferred by Jack: leave it unchanged.

Draft uses `desktop open/list` and the existing user config for `repo_root`; these
are accepted for implementation after Jack's October 8 review; the listed choices remain open.

## CLI deletion assumptions — October 8

Removing `run` preserves existing flow-first untyped lookup; explicit `skill` and
`flow` keep their own kinds. Task edges retain single-skill support and resolve in
the prepared Task checkout, not the caller's cwd. This does not settle the broader
collision-policy question. Historical request turn limits and the internal
prompt-parity helper remain readable; neither restores a public launch switch.

## History command shape — October 8

`history` defaults to the existing durable Work activity feed. `history list`
retains bounded Process pages and their exact cursors, while `history show`,
`replay` and `usage` move their existing readers/actions. No synthetic union or
second history store is introduced. `history list` is a reversible spelling
for the required command-page behavior, not a new product decision.

## Repository selection assumptions — October 9

Names contain no slash; paths use `/`, `./`, `../` or `~/`. `repo_root` must be
absolute or home-relative, avoiding caller-cwd-dependent configuration. Explicit
remote relative paths start at the remote login directory, not its saved repo.
These are reversible lookup choices, not shared Work identity or publication.
Primary and stack-parent Task selectors now carry repository-scoped full IDs through
dispatch; an unscoped ambiguous prefix still fails.

October 9 `lf context --skill realign --json`: generated goal 27,854/16,000 tokens
(11,854 over; generated inventory sizes vary). The complete launch source was read: Task/steer history plus 211
changed-file records. Authored-note curation cannot remove that generated payload;
the context-producer gap remains. No limit was raised.

## Reversible preview limits — October 9

Skill/inline context uses stored planning; launch may refresh Task comments.
Flow context uses the fresh-start snapshot choice below. Bare-agent/new operator-checkout previews
cannot prepare future checkouts. Remote previews are scoped to the selected Machine;
identity failure precedes assembly. Authentication is outside preparation; local
absence grants no first-start admission.

Task-run and opening explanation share launch owners; broader actions remain.
Opening JSON uses nullable `url`, not a receipt; unsupported platforms may show
a proposal beside a blocker. Omitted Task-run subjects use checkout/declaration
inference in both modes; naming a Flow requires the explicit Task positional form.

## Reversible exact-input choices — October 8

`desktop text/key` extend the existing pane event. Text rejects C0/C1 controls
(including newline/Tab), since ordinary terminal paste can submit them outside
bracketed-paste mode. Quotes, backslashes and Unicode remain literal; keys are
explicit named press/release actions. Active IME composition is temporarily
unavailable rather than discarded. This preserves Q3's existing-draft decision;
multiline insertion needs terminal-mode proof before expanding this contract.
Pre-input scratch: `/tmp/loo427-input-skDK4w/scratch/`; pre-compression scratch
and working diff: `/tmp/loo427-compress-input-nryd8M/`.

## Reversible composed-opening choices — October 9

`--diff` reveals the retained Changes browser without choosing a file or replacing
its draft/selection. The same Task link carries Session and companion intent.
`--session` opens only Task-associated Sessions in this cut; standalone Sessions
remain available through `session connect`. CLI opening does not run on the
Task's execution Machine automatically. Request-scoped native/Files readiness and failure propagation are implemented locally;
an `opening` reply grants no input target or usability. Composed `--diff` opening
clears zoom to make both requested panes visible; ordinary companion commands retain
zoom. Native endpoint acceptance remains unproved.

Repository/page readiness choice (October 9): registration of the validated repository
shell is usable even while planning loads. A plain Task request waits for its mounted
page, not optional Session preparation. Receipts are latest-per-locator/window,
not a durable history; locators never become repository identity. This reversible
endpoint definition leaves native usability and remote composition unproved.

## Reversible Flow preview presentation — October 9

`--context` shows the compiled graph with occurrence keys and one input state per
node. Only the initial skill/router gets current-snapshot input; later skills,
branch bodies and repeat passes are unavailable. Commands are labeled not-agent
and never executed, even when they could launch an agent. This previews a fresh
Flow, not an existing cursor. Sorted alternatives and backward edges retain the
shared graph's identity; no predicted routes or synthetic feedback are supplied.
Native skill arguments retain the original message, separately from router
instructions. This reversible choice is not Jack Heart's approval or complete-PR acceptance.
