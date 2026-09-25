# Direct Work-bound skills and flows

Finish: `lf --task LOO-291 design "…"` runs the design skill;
`lf --task LOO-291 code "…"` runs the authored flow in the Task checkout.
Each step receives the Task directive, exact Work attribution and fresh scratch.
Direct execution does not claim, bind, replace or advance the managed Task Flow.
`lf task run --flow …` continues to own that managed workflow.

Remove the `design`, `ship-5whys` and `wave` single-skill flow files. Wave
initialization invokes `wave/operate` directly. Keep multi-step workflows and
their explicit human boundaries. Bare names consistently prefer skills;
`lf skill NAME` and `lf flow NAME` resolve their requested kind directly.

Bound contributions leave edits uncommitted, including between flow steps.
Explicit authored operations retain their behavior. Human flow nodes still
need interactive acceptance; Task attribution alone does not create a durable
human Session. Failures stop subsequent steps.

Proof: isolated CLI execution with a local provider stand-in must show both
steps' exact Task context/cwd/Run identity, refreshed scratch, unchanged managed
Flow and no automatic commit of shared dirt. Exercise explicit name collisions,
design skill dispatch, flow failure, and Wave default initialization. Unit
resolution alone does not establish launch behavior. No live provider, Task
restart, installation, publication or PM change is needed for this correction.


## Review and result — 2026-09-24

Implemented both requested changes. One dispatcher now resolves and executes
bare, explicit skill and explicit flow invocations with an optional Work binding.
Deleted the name-specific precedence exception and both single-skill-only checks.
The existing loader already compiles a skill into a one-step invocation; no new
loader, Task worker, durable cursor, registry or compatibility alias was needed.
Removed all three single-skill builtin wrappers and updated the Wave default and
Product cron to `wave/operate`. Historical stored definitions are unchanged.

| Claim | Planned / implemented behavior | Proof | Result |
|---|---|---|---|
| Task-bound flows | Each skill uses the exact Task checkout, directive and Work subject | CLI launched from another checkout using `--task` and `--as`, bare and explicit flows | pass |
| Sequential context | Later skills see newly written scratch | Provider-input assertions before and after first step | pass |
| Independent contribution | Managed Task Flow stays unchanged; shared edits stay uncommitted and unstaged | Exact saved FlowPosition equality, unchanged HEAD and empty index | pass |
| Predictable names | Bare names prefer skills; explicit flow resolves the flow even with a same-name skill | Real CLI collision and design invocation, discovery checks | pass |
| Wrapper deletion | Skills remain available directly and to managed execution | Task selection, compiled invocation and default Wave tests | pass |
| Human boundaries | Explicit authored review gates retain acceptance requirements | Interactive acceptance tests; Task-bound batch flow refuses before provider or later step | pass |
| Documentation | User docs and injected guidance describe both paths | Six regenerated prompt snapshots and golden comparison | pass |

Final evidence: [12 CLI/flow tests](task-bound-flows-evidence/flow-tests.log),
[27 focused unit tests](task-bound-flows-evidence/unit-tests.log),
[golden comparison](task-bound-flows-evidence/golden-tests.log), and
[all-target clippy](task-bound-flows-evidence/clippy.log) pass. Cargo formatting
and diff whitespace checks pass. [Source hashes](task-bound-flows-evidence/receipt.json)
identify the tested contribution. No full suite is claimed.

Review narrowed the checkpoint change to Work-bound flows: unbound direct flows
and the resident step primitive retain their prior automatic checkpoints. The
first provider-input assertion exposed a fixture error: the fixture wrote through
LF_HOME, which provider children intentionally replace with LF_CONTROL_HOME, and
counted the version probe as a conversation. Corrected that fixture rather than
changing production Home handling. An initial golden comparison ran before
regeneration finished; the final comparison uses all six completed snapshots.

Read-only pipeline inspection through the local binary shows plain `design`,
while `task-design` and `launch-plan` retain their authored human nodes. The local
development Home reported an incompatible migration frontier; no configured
Task launch, store reset or promotion was attempted. Installed `lf` is unchanged.
This contribution remains local and does not publish or complete LOO-291.
Concurrent discovery notes and the broader canvas review remain their writers'
work; no final blanket checkpoint claimed them.
