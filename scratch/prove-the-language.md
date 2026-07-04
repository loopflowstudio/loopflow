# Prove the Language — three reference builds from goals

Asana item `1216257471904678` (= `wave/goals/3-vocabulary-completeness.md`).

## Problem

The goals wave's north star: *a developer writes one `GOAL.md` and the wave
builds the product.* The bar the GOAL.md states is sharp — **builtin steps and
flows expressive enough to build the clients and the server from goals with
zero step authoring.** If a product dev has to crack open `.lf/steps/` to
originate a product, the vocabulary failed.

Right now that bar is asserted, not tested. Nobody has driven the builtin
vocabulary against a greenfield build to see where it falls over. The roadmap
item *names* suspected gaps (scaffold, run, integrate) but on prediction, not
evidence. This work turns the assertion into an experiment: run the language
against the smallest real build, watch exactly where a developer would be forced
to author a step, and close those gaps precisely.

Who benefits: the product developer who wants to type a goal and get a running
CLI — and the language itself, which stops carrying atoms it only *thinks* it
needs and gains the ones a real build proves it's missing.

## Approach

**Falsify first, then build the atoms the probe confirms.** Three moves, in
order:

1. **The CLI probe** — the smallest build that can prove or falsify the
   language. Author a ~6-line `GOAL.md` for a tiny, real CLI in an *empty
   directory*, drive it with only builtin vocabulary, and log every point where
   the run demands an authored step. A CLI is the sharpest probe because it
   isolates the two most fundamental missing atoms (**scaffold**, **run**) from
   the client/server seam and from platform-build machinery. If the language
   can't build a CLI from a goal, it's falsified at its floor; if it can bar two
   atoms, we've *bounded the gap exactly* instead of guessing.

2. **Ship the atoms the probe confirms** — add `scaffold` and `run` as builtin
   `build/` steps (markdown prompts, auto-registered by `build.rs`), plus a
   `greenfield` builtin flow that chains them: the single "originate a product"
   hand a goal dispatches. Re-run the probe until it reaches a running CLI with
   `.lf/steps/` empty — **zero steps authored**.

3. **Sequence the two harder probes** — server (adds `run`-as-service +
   `integrate`, the client/server seam) and mobile (adds platform-build +
   `rams`-as-step) are genuinely distinct slices, each surfacing a different
   atom. File them on the roadmap with their predicted primitives; land them as
   their own probes after the CLI slice proves the method.

This PR lands move 1 + move 2 (the CLI slice: probe script, `scaffold`/`run`
steps, `greenfield` flow) and files move 3. The CLI slice is one coherent chunk
— it's the whole language-floor, not a fragment.

### The atoms (precise specs)

All three are builtin `build/step/*.md` prompts — **prompts, not scripts**.
Platform knowledge (cargo vs uv vs xcodegen) lives in agent judgment guided by
the GOAL, exactly like every existing step. Adding one is dropping a markdown
file; `build.rs` registers it (confirmed: `build/step/` is auto-scanned, no
manual wiring).

**`scaffold`** — stand up a greenfield project from nothing.
```
requires: GOAL.md describing the target surface
produces: a runnable skeleton that builds and prints hello; committed
default_agent: codex
action_style: procedural
```
Reads the GOAL for surface + platform, picks idiomatic tooling (`cargo new`,
`uv init`, `npm create`, SwiftPM/xcodegen), creates the *minimal* skeleton that
**compiles and runs**, and — critically — **owns the greenfield git setup the
other steps assume**: if there's no repo/branch/main, it inits them. `scaffold`
does not design features; it produces the ground `design`/`implement`/`demo`
stand on. This is the atom that fixes "everything today assumes an existing
repo + design doc." `init` (sets up loopflow) is unchanged and orthogonal.

**`run`** — build and actually execute the artifact; report observed behavior.
```
requires: buildable artifact on branch
produces: scratch/run-observations.md (observed behavior vs. the GOAL's "done when")
default_agent: codex
action_style: procedural
```
Builds, then executes in the artifact's native modality — CLI: invoke the
commands; server: start it + hit endpoints; mobile: simulator build + launch —
and captures *actual output*, asserting pass/fail against the GOAL. This is the
loopflow-native, headless, **chainable** equivalent of the `/verify` skill:
where `verify` is an ad-hoc Claude skill and `demo` is an interactive human
narration, `run` is a step any flow drops before `gate`. One implementation —
`run` is not folded into `demo` (different audience, different mode).

**`integrate`** — exercise a client against its server (the defining seam).
```
requires: client + server artifacts
produces: scratch/integration-report.md
default_agent: codex
action_style: procedural
```
Brings up the server, points the client at it, drives the round-trip, asserts
the response. Distinct from the existing `integrate-upstream` (git-merge of
upstream main — unrelated). **Deferred to the server probe**, not this PR; its
full cross-repo form forks with `3-wave-repo-split`. v1 handles same-repo /
two-local-dirs.

### The `greenfield` flow

```yaml
# Originate a product from a goal, end to end.
- scaffold      # empty dir -> runnable skeleton + repo/branch
- design        # now has a repo to design within
- implement     # design doc -> code + tests
- run           # execute; observe real behavior
- gate          # make it shippable
```
This is `design-and-ship` with `scaffold` prepended (supplying the branch +
skeleton the flow used to assume) and `run` inserted before the gate. A goal
dispatches `greenfield` as its "originate" hand; `ship-roadmap` and the other
goals stay untouched.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Do the gaps actually exist, or is this prediction? | Direct inspection: **no** `scaffold`/`run`/`integrate` builtin step. `design` opens "ask what they want to build" (interactive, assumes a conversation); `implement` `requires: scratch/<branch>.md`; `demo`/`code-review`/`gate` `require: diff vs main`. Every build/ship flow assumes a branch off `main`. Confirmed, not guessed. | The probe validates a known prediction; atoms are the fix. Bounds risk to prompt quality, not discovery. |
| Is adding a builtin step hard? | No. `build.rs` scans `build/step/*.md` and generates the registry (`builtins.rs:196`); dropping a file is the whole install. | Atoms are cheap markdown; ship all in one PR. |
| Can one `scaffold` step handle Rust CLI *and* mobile *and* server? | Steps are prompts, not scripts (every existing step is a prompt). `scaffold` guides an agent to pick tooling from the GOAL; nothing is hardcoded. | `scaffold` stays a single step; platform-specificity is agent judgment. |
| Does greenfield break git machinery (no branch, no `main`, no PR base)? | build/ship flows + `lf op` assume a branch off `main`. A truly empty dir has none. | `scaffold` explicitly owns repo/branch init — "make the ground the other steps assume." The probe runs in a scratch dir, never this repo. |
| Isn't `run` just `verify` or `demo`? | `verify` is a `/`-skill (ad hoc, human-invoked); `demo` is `interactive: true` (human present, narrates); neither is a non-interactive step producing a chainable observation artifact. `run`/`verify`/`rams` resolve as steps *only* via the `load_agent_skill` fallback today. | `run` is a distinct builtin step producing `scratch/run-observations.md`, chainable before `gate`. No fold, no shim. |
| Where does the probe run — does it pollute loopflow with product code? | The probe runs in a throwaway temp dir via `scripts/prove_language_cli.sh`. Only the script + the new builtin steps/flow land in this repo. | Safe, re-runnable, no CLI product code enters the loopflow tree. |
| Is `integrate-upstream` already the seam atom? | No — it merges upstream `main` into a branch (git), used by `ops/flow/sync.yaml`. The client/server seam is genuinely absent. | `integrate` is a new atom, deferred to the server probe. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Build all three reference products to polish as the deliverable | Maximally concrete | Most of the work is product code that doesn't advance the *language*; not falsification-first; a huge PR that buries the gap findings. The point is the vocabulary, not three toy apps. |
| Paper audit only — read the steps, declare the gaps, add atoms, done | Fast, no runtime | Violates the item's own framing (an *acceptance test*). Misses gaps that only surface at runtime — does `gate` actually work with no prior CI? does `design`'s interactivity stall a headless greenfield run? The inspection here is the *prediction the probe must validate*, not a substitute for it. |
| Add `scaffold`/`run`/`integrate` speculatively, skip the probe | Ships atoms sooner | Risks building the wrong atoms or wrong shapes. The probe costs one scripted run and bounds the gap to the exact primitives needed. Cheap insurance against a mis-specified atom. |
| Start with the server or mobile probe | Covers the "real product" seam sooner | Server drags in process lifecycle + the `integrate` seam; mobile drags in simulator/platform-build + `rams`. Both entangle the fundamental atoms (scaffold, run) with surface-specific machinery. The CLI isolates the floor. |
| Promote the existing `verify`/`run` skills to steps instead of writing `run` | Reuses prose | The skills are Claude-Code-harness-shaped (interactive affordances, skill frontmatter); a builtin step is a clean loopflow prompt with `requires`/`produces`. Promoting drags harness assumptions in. Write the step; let the skill stay a skill. |

## Key decisions

- **Falsify before build.** The probe is one scripted run; it converts "we think
  scaffold and run are missing" into "the CLI build authored N steps, here they
  are." We don't add atoms on prediction alone.
- **CLI is the floor probe.** It isolates `scaffold` + `run`. Server and mobile
  are follow-on probes, each its own item, because each surfaces a *different*
  atom — that's the honest split, not incrementalism for its own sake.
- **Atoms are prompts, not scripts.** One `scaffold` step covers every platform
  because it guides an agent, not a hardcoded toolchain. This is the whole
  reason the vocabulary can stay small.
- **`scaffold` owns greenfield git.** The single hardest hidden assumption —
  every flow expects a branch off `main` — is resolved in one place: `scaffold`
  makes the repo the rest of the vocabulary assumes.
- **`run` is a first-class atom, not a folded `demo` or a promoted skill.**
  Headless, chainable, produces an observation artifact, feeds `gate`.
- **One `greenfield` flow, `ship-roadmap` untouched.** The goal gains a new hand
  to dispatch; the loop doctrine doesn't change.
- **`integrate` deferred, not dropped.** Precisely specified now, built with the
  server probe, cross-repo form forks with `3-wave-repo-split`.

## Scope

- **In scope:** the CLI probe (`scripts/prove_language_cli.sh` + its `GOAL.md`);
  `scaffold` and `run` as builtin `build/step/*.md`; the `greenfield` builtin
  `build/flow/greenfield.yaml`; a builtin-registry test asserting all three
  register; docs update (`docs/wave-authoring.md` + the vocabulary README) to
  list the new atoms and the greenfield flow; roadmap write-back of the CLI
  result and the two follow-on probes.
- **Out of scope:** the `integrate` step implementation (specced here, built with
  the server probe); the mobile platform-build step + `rams`-as-step (the mobile
  probe); building any of the three products past "runs and is demoable"; the
  cross-repo form of `integrate` (forks with `3-wave-repo-split`); pruning
  existing flows (that's the sibling `3-prune-flow-vocabulary`).

## Done when

- `scripts/prove_language_cli.sh` runs green: from an **empty directory** + a
  ~6-line `GOAL.md`, the `greenfield` flow scaffolds, designs, implements, runs,
  and gates a working CLI; the script prints the built binary's **real output**
  and asserts the probe dir's `.lf/steps/` is empty — **0 steps authored**.
- `scaffold` + `run` builtin steps and the `greenfield` flow resolve
  (`get_builtin_step`/`get_builtin_flow` and the registry test pass).
- `cargo test` and `uv run pytest python/tests/` pass.
- Asana `1216257471904678` carries the CLI result; the server-probe (`integrate`)
  and mobile-probe (platform-build + `rams`) are filed with their precise
  missing primitives via `lf op pm update`.

## Measure

The item's own metric: **vocabulary gap count against the acceptance test.**

- **Baseline (today, CLI surface):** run the probe against the *unmodified*
  builtin vocabulary and count authored-step demands. Prediction: **2**
  (`scaffold`, `run`). The run produces the real number — that's the falsifiable
  result. Command: `scripts/prove_language_cli.sh --baseline` (drives builtin
  vocab only, logs each point the flow stalls for a missing atom).
- **Target (CLI surface, after atoms):** **0** authored steps; the CLI builds
  and runs from `GOAL.md` alone.
- **Across all three surfaces:** gap count = 0 is the finish line for the whole
  item — CLI (this PR), then server (`integrate`), then mobile (platform-build +
  `rams`). Each probe reports its own gap count on the roadmap.

"Better" = the number of `.lf/steps/*.md` a developer must write to get a
running product goes from >0 to 0.
