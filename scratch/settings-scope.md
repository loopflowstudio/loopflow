# Earlier settings implementation — LOO-714

Jack Heart accepted choice-before-fallback on November 3, 2026. The complete
prior design and verification record is `da937b7f5a:scratch/settings-scope.md`.
The current implementation plan is settings-catalog.md; it supersedes the old
DB-storage and catalogue/portability exclusions. Review-only PR #1805, Auto-Ship
stopped. Automatic limit recovery remains PR2; the whole Task is unfinished.

Pre-implementation design evidence remains in
`/tmp/loo-714-preserved/designs-20261102.tgz`; the four identical historical
pointers are preserved at `da937b7f5a:scratch/design-questions.md` (also
route-design.md, storage-model.md and task-brief-history.md). Their November 2
storage exclusions are superseded by settings-catalog.md.

Preserve every applicable agent choice ahead of every default_agent fallback,
then source > scope > target within each class. Personal skill choice beats
repository occurrence choice; personal skill fallback yields to repository choice.
Occurrence identity is defining Graph canonical name plus authored local step ID,
retained through composition, repetition, loops and export. Anonymous steps use
skill/general settings. Duplicate IDs within one definition fail.

Session/resume -a applies to that conversation, not Task policy or agent-issued
child commands. Explicit Graph/Task execution -a reaches steps and routers.
No built-in per-skill harness recommendations or per-skill accounts. Native resume
retains account/home/provider continuity. A same-provider fallback may refresh a
model; cross-provider import requires an explicit choice (assumption in questions.md).

False, empty, omitted and unset are distinct. docs/context/exclude append; [] clears.
Planning replaces as a tagged unit; personal transport cannot assign a repository
Team. summaries/summary_tokens are parsed but inactive. Other supported combinations
and the remaining extension acceptance live in settings-catalog.md.

Earlier implementation: InvocationSettings carries CLI intent; SkillInvocation keeps
Graph origin; captured recipes load skills from the launch repository. TaskProcessOptions.agent,
Task agent policy and duplicate scope/merge paths were removed. The single unreleased
settings_authority.sql migration is this Task's migration. Released aliases remain
with their existing named removal Tasks.

Check before this extension: 909 affected Rust tests ran, 908 passed; the remaining
Linear fixture was isolated from live personal preferences and its two focused tests
passed. Architecture, website (72), headless Swift (664 plus isolated lifecycle),
inventory, formatting, migrations and Clippy passed. Exact commands and logs are in
`da937b7f5a:scratch/settings-scope.md`. These are prior-head evidence, not proof of the
file-backed extension. Hosted checks were skipped while scratch was retained.

PR2 proof still needed: weekly-limit rejection on A continues on B with a fresh
provider thread and the same LF Session/work; old thread attribution remains A.
Cover interactive Sessions, headless Graphs and all-accounts exhaustion/reset waiting.
The router and retry need one selection decision. No history rewrite, retry-invariant
removal or transcript symlinking is authorized. Reproduction remains in limit-failures.md
and weekly-limit-2026-11-02.md. Unrelated critique artifacts are preserved.
