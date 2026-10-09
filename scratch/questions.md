# Product decisions still open

The [plan](compare-cmux-s-command-line.md) owns implementation; [findings](findings.md)
own evidence. Jack Heart has already selected one shared Work tree, subtree
delegation, one repository window across machines, macOS-only view control and
one PR. Those choices are settled. Recorded location, local inspection/arrangement and
exact-target passive read transport are local; peer composition and native I/O
remain unfinished.

## Selected decisions — October 8–9

Jack Heart selected Q1: execution-Machine-local operations and Git-ref planning
exchange (406 writer/Linear, 412 exchange); imported completion never moves
Workflow or cleans execution. Q2: nearest explicit assignment for unstarted Tasks;
narrower overrides win, started Tasks stay put, unknown historical provenance stays
unknown. Q3: insert at the cursor into the existing draft, with Enter separate.
The plan owns implementation; exchange/admission and native fidelity remain unproved.
Original Q1–Q3 record: `7d5bcf939:scratch/questions.md`, those headings.

## Reversible visibility choice — October 8

Hide/restore address an existing retained view through the selected plan/window,
Machine/checkout and pane/content occurrence. Restore removes collapse without
selecting Work, focusing a window, changing zoom or acquiring a client. View occurrence tokens grant no
terminal-surface or composer-input authority.

## Reversible arrangement choices — October 8

An empty split, move and resize leave pane selection and zoom unchanged. Move
relocates a retained leaf after another exact pane (right for vertical, below for
horizontal), within one Machine/checkout; it never reassigns Work. Resize names
two exact panes and the target side's share of their separating divider. Zoom
changes visibility only; focus explicitly selects/reveals the exact pane in its
workspace, without switching Work or foregrounding the window. No view operation
authorizes native input.

## Reversible companion choices — October 8

CLI companions preserve focus, zoom and Work selection. An empty exact target is
filled; occupied content is never replaced. Files/Flow-log reuse and unhide the
named Task's pane only in its recorded/prepared checkout. Shell always allocates
and remains local-only until remote opening is composed. lf3 supplies fixed-buffer extraction; native acceptance remains.

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

October 9 context: complete launch source
`.lf/tmp/context/4af9d84cf4443dfe4f2aa9a3253951a06605c1da0f3f90806c6de2e56a4bbfa8.md`
was inspected. `lf context --skill compress` reports 32,671 generated Work-seed
tokens (16,671 over), from the manifest/historical steers. Authored notes fit;
curation cannot change that producer gap without rewriting historical input or
raising budgets. Neither is authorized.

## Explicit repository association — October 9

October 9 reversible choice: `repo identity --bind ID` explicitly selects the
supplied repository identity even after local Work exists. Prior IDs remain local
locators in the same `repository_plans` table; Work, provider mappings, uncertain
effects, journals and execution stay unchanged. It selects/publishes no destination.
An ID already locating another local checkout is not reassigned. This associates
two Machines' established roots, not two same-Machine stores or duplicate Work.
Divergent Task/Project IDs now have separate explicit correspondence/full-ID lookup;
joint projection now uses explicit correspondence. Binding alone grants none and never rekeys effects.
Desktop reassociation on open/restore is local; delegation exchange and peer
first-start admission remain. Neither binding nor a retained alias authorizes start; Git-selected Tasks
without retained checkouts still refuse.
LOO-412's started-Task source-transfer path conflicts with Jack Heart's retained-
Machine decision and is excluded; dependent remote launch work stays unfinished.

Scene retention choice (October 9): a validated local locator keeps its SwiftUI
scene key across binding. The current plan and input token change without replacing
its native owner. An unopened restored alias redirects before mounting any panes.
Identity is refreshed on opening/restoration, not by a new timer.

## Reversible preview limits — October 9

Skill/inline context uses stored planning; launch may refresh Task comments.
Flow context uses the fresh-start snapshot choice below. Bare-agent/new operator-checkout previews
cannot prepare future checkouts. Remote previews are scoped to the addressed Machine;
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
