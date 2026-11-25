# Direction — LOO-1096 (Breakpoints)

## Exact words (Jack Heart, 2026-11-24)

Original problem:
> right noe task sessions will say things like "waiting for you at ship-review" instead of seeing oh were at ship-review, thats an interactive step, ok lets get started

Explore & pivot to Breakpoints (`scratch/explore.html`):
> ok i think i changed my mind; i think we should move to the model where instead of interactive steps, we have breakpoints. we might have to slightly rewrite some prompts

> the nice thing about that is then you can have models other than the session's model do the work

> I think we still want some amount of this, because you can still do lf X to get an interactive X session

> Right. I think its better if we try to not have 3 different cases in the skills though, and have the headless steps basically propose a plan of action that would continue if i dont intervene

> 1a
> 2a

> i also wonder how this interacts with 1078; should breakpoints be "Paused" state, i think maybe it should be the same

> 1a 2a

> ok so there is a big diference in that Paused right now is meant to mean like shelved an put in a lower pri part of the sidebar. we want the opposite for these tasks -- these are the firsto nes you should be talking to

(Where the first `1a / 2a` chose: repurpose LOO-1096 for Breakpoints, and wake/open the Task conversation in a held terminal at a breakpoint to present what the preceding step produced. The second `1a / 2a` chose: keep Breakpoint `Active` separate from LOO-1078's `Paused` shelved state, and use standalone `- break` YAML items.)

## Settled model

1. **Replace `interactive:` steps with Breakpoints (`- break`):**
   - Every skill (`explore`, `ship-review`, `verify`, `design`, `research`, `demo`) is an ordinary Graph step running headlessly on its own configured `agent:` / model (with quota fallthrough), independent of the Task conversation's model.
   - Standalone `- break` (or named `- break: <name>`) is an explicit pause point between steps in a Graph (e.g., `explore → break → plan → build → ship-review → break → deliver → verify → break`; `research.yaml` collapses to `loop: [ research, break ]`; `kickoff-demo.yaml` becomes `pursue → demo → break`).
   - `auto: on` and `lf -b` run straight through breakpoints; `lf task run` continues past the current breakpoint.
   - Keep backwards-compatible parsing of existing `.lf/graphs/*.yaml` and captured `task_graphs.graph` JSON containing `interactive: <skill>` (desugared as `<skill>` followed by a breakpoint) until released `lf` ships, marked with a filed removal Task per `AGENTS.md`.

2. **Two skill modes only (interactive vs. headless):**
   - **Interactive (`lf <skill>`):** someone is in the conversation directly; collaborate as before.
   - **Headless (Graph step):** do the work, produce the artifact/checks (`scratch/explore.html`, `scratch/ship-review.html` + `tour.yaml`, `scratch/verify.md`, `scratch/demo.md` / built `.dev` bundle without focus-stealing popups), pose numbered questions, and **propose the default plan of action that continues if nobody intervenes**. The skill never needs to know whether a breakpoint sits after it: if a breakpoint follows, the Graph pauses before executing the next step; under `auto: on` / `-b`, the Graph continues on that default path.

3. **Task conversation wakes / opens held at a breakpoint (2a):**
   - When the Graph pauses at a breakpoint, wake the Task's primary conversation—or start it in a held terminal if closed/quit (see reference commits `83232c9b4d` and `3e59fc7d1d` preserved at `13c443c955` on `origin/jack-heart/ba45eb-interactive-step-should` for held-terminal wake & arrival tracking).
   - Hand the conversation a breakpoint handoff (not a passive `update:`): present what the preceding step produced—summary, artifact links, proposed default continuation, and numbered questions—so the conversation never stops with *"Waiting at breakpoint."* Once the person answers, record their words and continue (`lf task run` / `--from <step>`).
   - No `lf help <skill>`, no `task-session` cosplaying `ship-review`, no special `opening_step` / `Waiting("explore")` hack on Task creation (`feature` Tasks immediately start running `explore` as step 0 and pause at the breakpoint after `explore`!).

4. **Interaction with LOO-1078 (`lf task pause`):**
   - Keep **At Breakpoint** distinct from LOO-1078's **`Paused`** (shelved, lower-priority sidebar group): a Task at a breakpoint is the opposite—highest priority in **`Active`** (and surfaced in **Now**), the first ones you should be talking to—and `lf task run` / `lf task auto on` moves straight past it. `lf task pause` composes cleanly on top to shelve a Task into `Paused`.

## Reference history

- Previous `interactive:`-handoff branch commits (Passes 1–5) are preserved at commit `13c443c955` (`origin/jack-heart/ba45eb-interactive-step-should`), including:
  - `3e59fc7d1d` — `task_graphs.arrival` + scoped handoff delivery so repeated loops (e.g. Loop 2 at `ship-review`) re-trigger once and stale handoffs hide automatically when the Task moves on.
  - `83232c9b4d` — starting a quit Task primary conversation in a held terminal on arrival.
- Branch was reset cleanly onto `origin/main` (`80cced1bd4`) so this implementation builds Breakpoints cleanly from `main` and deletes `interactive:` machinery rather than carrying forward unmerged `interactive:` handoff code.
