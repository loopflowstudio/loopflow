# Rename `roadmap/` → `wave/`

## Problem

`roadmap/` is a lie. It implies a document you read — a plan that lives separately from execution. But in loopflow, the items in `roadmap/` are the execution specs. They have frontmatter. They get ingested. They drive waves. The plan and the config are the same artifact.

`wave/` says what they are: units of work that agents run. The rename unifies the conceptual model. No more explaining that "the roadmap is actually the wave config" — the directory name does that work.

This also sets up wave-specs-launcher cleanly. When items declare `flow:`, `direction:`, `area:` in frontmatter, they live in `wave/` — because that's what they are.

## Approach

Mechanical rename across the entire codebase. No behavior changes, no new features. The directory structure, Rust path logic, prompt content, doc references, CLI help text, test fixtures — all switch from `roadmap/` to `wave/`.

### Layers (in dependency order)

**1. Rust engine — path logic (`prompt.rs`, `config.rs`)**

`gather_docs()` in `prompt.rs` builds paths like `repo_root.join("roadmap").join(wave_name)`. Change to `repo_root.join("wave")`. Update the `Document` category from `"roadmap"` to `"wave"`. Update the `GatherContextOpts` comment.

Config comment in `config.rs` says `"Include reports/, roadmap/, scratch/"` — update to `wave/`.

**2. Builtin prompts (6 steps, 6 flows)**

Steps that read/write `roadmap/<wave>/`:
- `plan/ingest.md` — picks items from `wave/<wave>/`
- `plan/roadmap.md` — produces items for `wave/<wave>/`
- `ops/add-to-roadmap.md` — routes scratch/ to `wave/<wave>/`
- `ops/update-roadmap.md` — revises `wave/<wave>/` after shipping
- `interactive/design.md` — references `lf add-to-roadmap`
- `LOOPFLOW.md` — "Don't modify `roadmap/`" → "Don't modify `wave/`"

Flows referencing roadmap:
- `plan/roadmap-reduce.yaml`, `plan/roadmap-polish.yaml`, `plan/roadmap-expand.yaml` — rename to `wave-reduce.yaml`, etc.
- `code/ship-roadmap.yaml` — rename to `ship-wave.yaml`
- `plan/publish.yaml` — references `add-to-roadmap` step

Step/flow names that include "roadmap":
- `add-to-roadmap` → `add-to-wave`
- `update-roadmap` → `update-wave`
- `roadmap` (the step) → `wave-plan` (avoids collision with the `wave/` directory concept)
- `ship-roadmap` → `ship-wave`
- `roadmap-reduce` → `wave-reduce`
- `roadmap-polish` → `wave-polish`
- `roadmap-expand` → `wave-expand`

**3. Rust builtins registry (`builtins.rs`)**

Update `include_str!` paths and HashMap keys for renamed steps and flows.

**4. Custom project steps (`.lf/steps/`)**

- `ux-review.md` — produces items in `roadmap/concerto/` → `wave/concerto/`
- `ux-synthesize-concerto.md` — reads from `roadmap/conductor/` etc. → `wave/conductor/` etc.

**5. CLI and binary args**

- `lf/mod.rs` — `--lfdocs` help text mentions `roadmap/`
- `bin/lf-prompt.rs` — same

**6. Documentation**

- `README.md` — flow tables, step tables
- `PROMPT_STYLE.md` — action goals reference `roadmap/`
- `docs/index.md`, `docs/config.md`, `docs/lf.md`, `docs/lfops.md`
- `CLAUDE.md` — `scratch/` section mentions `roadmap/`

**7. Tests**

- `tests/context_tests.rs` — creates `roadmap/<wave>/` fixtures → `wave/<wave>/`
- `tests/flow_tests.rs` — tests `roadmap-reduce` expansion → `wave-reduce`
- `tests/goldens/*.md` — golden prompt output mentioning `roadmap/`

**8. The directory itself**

`git mv roadmap/ wave/`

### Backwards compatibility: none

Per CLAUDE.md: "Don't maintain backwards compatibility unless explicitly required." This is an internal directory convention. No external consumers. Clean break.

### Step/flow name rename rationale

The step currently called `roadmap` synthesizes analysis into a sequenced plan. Renaming it to `wave-plan` avoids ambiguity — `lf wave-plan` means "create a wave plan", not "do something to the wave/ directory."

`add-to-roadmap` → `add-to-wave` is direct. `update-roadmap` → `update-wave` is direct. The `-roadmap` suffix on flows becomes `-wave`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Rename dir only, keep step/flow names | Less churn | Half-renamed is worse than not renamed. "roadmap" in step names while the dir is `wave/` is confusing. |
| Add symlink `wave/ → roadmap/` for transition | Gradual migration | Single user, no external consumers. Symlinks add complexity for zero benefit. |
| Rename to `backlog/` instead of `wave/` | More conventional term | Misses the point. The items aren't just a backlog — they're executable wave specs. `wave/` reinforces that they drive agent execution. |
| Keep `roadmap/`, rename nothing | Zero effort | The conceptual mismatch compounds. Every new user/contributor has to learn that "roadmap" means "wave execution config." |

## Key decisions

**Full rename including step/flow names.** The wave's principle of "single source of truth" applies to naming too. If the directory is `wave/`, the steps that operate on it should say `wave`, not `roadmap`.

**`wave-plan` not just `plan` for the roadmap step.** `plan` is too generic and collides with the `plan/` step category. `wave-plan` is unambiguous.

**No migration code.** This isn't a published API. `git mv` + find-and-replace. Anyone with a `roadmap/` directory after this change sees a clear error and knows what to do.

**Golden test updates are part of scope.** The golden prompt tests verify exact output. They'll break and need updating — that's correct behavior, not a problem.

## Scope

- In scope: every reference to `roadmap/` in the codebase — Rust, prompts, docs, tests, the directory itself
- In scope: renaming steps/flows that include "roadmap" in their name
- Out of scope: new features, behavior changes, wave-specs-launcher work
- Out of scope: Python client code (doesn't reference `roadmap/` directly — it reads from the daemon API)

## Done when

```bash
# No references to old paths remain
rg 'roadmap/' --type rust --type md --type yaml | grep -v 'target/' | wc -l
# → 0

# Directory exists
ls wave/

# Old directory gone
ls roadmap/ 2>&1 | grep -q "No such file"

# Tests pass
cargo test --all
```
