# Complete the per-wave Linear team migration — binding PR

## Problem

Per-wave team support and reteam mechanics shipped, but the three GOAL.md team
bindings never merged to main. All three waves still bind to the shared team
`60558c53` (W2), so new Projects and Tasks keep receiving W2 identifiers and
`lf pm reteam` points the wrong way. This is the config that makes ENG/SCI/PRD
ownership real.

Who benefits: every wave gets its own identifier namespace, and the known
"`lf pm task done` cannot close an ENG-* issue because the wave binds to W2"
class of failure (wave-team vs issue-team split) goes away.

## The demo

`lf pm reteam --wave <w>` (dry run) now prints the wave-owned target:

- infrastructure → `08c8d501… (ENG-*)`
- intelligence → `7de894bd… (SCI-*)`
- product → `e894ffa1… (PRD-*)`

…and defers the three live-body Tasks (W2-319/320/321) as protected writers
instead of renumbering them. Ran all three; output captured below.

## Approach

Change only the three `pm.linear_team` values in the wave GOAL.md frontmatter.
Nothing else — initiative ids, crons, definitions, and every other field are
untouched. Reproduce the minimal binding change on this fresh branch; do not
pull `origin/jack-heart/ux` history.

The provider apply (`lf pm reteam --apply`) is **not** part of this PR. It runs
from the supervising flow after W2-319, W2-320, and this Task are terminal, so
no live body writes an identifier being renumbered.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Can a data-only UUID swap break parsing? | No — the key `pm.linear_team` and YAML shape are unchanged; only the value differs. | Parser tests unaffected; end-to-end dry-run is the real proof. |
| Do the dry-runs actually resolve the new teams? | Yes. All three print `(ENG-*)/(SCI-*)/(PRD-*)` reading the edited GOAL.md via the real parser + Linear key lookup. | Confirms the binding is live and correct. |
| Is running a dry-run from this body safe while W2-319/320 run? | Yes — dry run performs no mutations; `ensure_reteam_apply_safe` only fences the `--apply` path. Live bodies show as deferred protected writers. | Safe to demonstrate; apply stays blocked. |
| Does the binding change renumber anything by itself? | No. Renumbering happens only under `--apply`. | Binding PR is inert and mergeable while bodies run. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Cherry-pick the three commits from `origin/jack-heart/ux` | Carries unrelated history | Directive forbids; a fresh minimal branch is cleaner. |
| Bind + apply in one PR | "Finishes" faster | Violates the live-writer fence; would renumber under active bodies. |

## Key decisions

- Binding only; apply is a separate, human/flow-gated phase after terminal
  settlement of the three infrastructure bodies.
- Verification is the live dry-run against the edited files, not a new unit
  test — a data-only edit needs no code test, and the dry-run exercises the full
  parse→resolve path.

## Scope

- In scope: the three `pm.linear_team` value changes; dry-run verification.
- Out of scope: `reteam --apply`, new teams, initiative/Project definition
  changes, renaming completed W2 issues, copying `jack-heart/ux` history.

## Done when

- `git diff --stat` shows exactly the three GOAL.md one-line changes.
- `lf pm reteam --wave {infrastructure,intelligence,product}` each print the
  wave-owned team `(ENG-*/SCI-*/PRD-*)` and defer live bodies. (Verified.)
- PR merges; Task completes. Apply phase is a separate follow-up gated on
  W2-319/W2-320/W2-321 being terminal.

## Verified dry-run output

```
wave/infrastructure → team 08c8d501-791d-4db0-a14c-15599d365955 (ENG-*)  [dry run]
  deferred — protected writing Task body (3): W2-320, W2-321, W2-319
  left as historical: 64 completed issue(s) stay in the shared team
wave/intelligence  → team 7de894bd-17da-403a-9d71-93c40afe9367 (SCI-*)  [dry run]
wave/product       → team e894ffa1-bc38-4382-af89-2e1d89884f4e (PRD-*)  [dry run]
```
