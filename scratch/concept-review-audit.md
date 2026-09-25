# Concept-review audit

2026-09-25. Bounded documentation contribution, based on `scratch/loopflow.md`,
`scratch/concept-review-skill.md`, and `scratch/concept-review.md`. Initial HEAD:
`87b14431d`. Runtime files are changing concurrently under the main agent.

## Result

Revised usage documentation first, then the portable skill and PROMPTS guidance.
The review starts with a concrete interaction and recovery path, allows the
human to reconsider the design, and follows product gains through types/APIs
and infrastructure. Clear usage can stay unchanged; deletion is not a target.

Fixed three gaps:

- Task-specific verdict wording excluded ordinary loopflows. The skill now
  follows the decision protocol assigned to its occurrence, without inventing
  commands or requiring tracked work.
- An attractive proposal could displace accepted requirements or reuse stale
  behavior proof. Alternatives remain proposals; affected claims return to
  implementation and behavior review before completion.
- The history document could read as a completed continuation repair. It now
  distinguishes the user's reported gap, accepted loopflow direction, source
  evidence, and implementation proof still owed.

## Checks and sources

- `git diff --check -- PROMPTS.md docs/concept-review.md
  rust/loopflow/src/engine/builtins/task/skill/concept-review.md` passed.
- Read-only `uv run python`/PyYAML inspection passed skill frontmatter and found
  all four direct review-slice successors: build, slice, task-gate, pursue.
  Each is concept-review. Pursue assigns the decision to concept-review.
- Portability inspection found no linked dependencies; the sole literal path
  is the produced `scratch/concept-review.md`. An initial overbroad substring
  check falsely treated the phrase “docs/skills” as an input path; inspection
  of literal paths and links corrected the check without changing the skill.
- `rust/loopflow/build.rs` scans `task/skill/*.md` into the flat builtin catalog;
  `engine/builtins.rs` reads the generated catalog. This is source evidence of
  discovery, not a rebuilt/installed CLI check.
- Re-read the four merged commit messages containing PR descriptions and their
  selected patches with `git show`. Verified net totals with commit stats:
  #872 / `309575f8e`: 40,201; #1099 / `a7044e2b5`: 14,779;
  #1237 / `5f7f66833`: 39,931; #1270 / `e0849bad4`: 1,498.
  The earlier first-parent ranking was retained, not rerun.
- Patch evidence: #872 `rust/loopflow/src/{lib.rs,ops/mod.rs}` removes reachable
  surfaces; #1099 `store/migrations/0.11.036_delete_sessions.sql` transfers facts
  and references before dropping Session tables; #1237 `run_record.rs` defines
  launch records and exclusive terminal settlement; #1270 `docs/lf.md` removes
  the default feature cycle and documents finite Flow completion.

## Adversarial review

| Case | Required judgment in the revised skill |
| --- | --- |
| Behavior passes and model is clear | Keep it; no ceremonial rewrite or extra pass |
| Product improves but code grows | Evaluate the experience and preservation proof |
| Concepts look clean but review-slice has a gap | Retain the gap; no completion |
| Proposed simplification changes approved behavior | Preserve the proposal and name the required human choice |
| New model makes prior proof stale | Return affected claims for implementation and review |
| Standalone occurrence has no decision protocol | Save judgment; invent no Task or successor launch |

## Remaining integration evidence

No documentation blocker remains within these four owned files. Main agent owns
Flow/runtime integration, the exact protocol, and ordinary/bound loopflow parity.
The existing `controller/task/mod.rs` fixture
`driver_runs_fresh_slice_turns_until_the_flow_finishes` contains two four-step
passes and saved-verdict recovery with simulated provider/PM transport; its
source was inspected, not executed here. It cannot establish standalone Flow
parity, live human handoff, or preserved installed invocations.

Only the four assigned documentation files were written. No tests added,
runtime suite run, commit, publication, worker launch, or installation performed.
