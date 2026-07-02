# `--docs`: one explicit doc-prefetch flag, no ambient repo docs

Kill the *automatic* repo-doc dump: `lfdocs`, the `<lf:docs>` section, and the
always-on root-`*.md` glob all go away. What dies is the automatism, not the
capability — the old behavior is reproducible on demand as `--docs '*.md'`.
`scratch/` and `wave/` graduate to first-class ambient sections (no flag).
`--area` is generalized and renamed to `--docs`, an explicit prefetch flag for
pointing at a file, glob, or directory.

`--docs '*.md'` simulates old-`lfdocs`: glob semantics make `*.md` root-only
(matches `README.md`, not `docs/foo.md` — use `**/*.md` for recursive), so it
reproduces exactly the old root-level dump. Combined with now-ambient `scratch/`,
`--docs '*.md'` is the full old `lfdocs=true` behavior — automatic became explicit.

Note the naming trap: the new `--docs` flag is the mechanical successor to
**`--area`** (file/glob/dir loading, reusing `gather_area_docs`), not a rename of
`lfdocs`. Despite the shared word, `--docs` does not control any `<lf:docs>`
section — that section no longer exists; `--docs` content renders as ordinary
included files.

## Goal

Shrink the ambient prompt and make context loading explicit. Same direction as
`RLM → OPERATE` (make guidance opt-in). Today every `lf` run silently dumps
every root-level `*.md` into a `<lf:docs>` block. Repo docs should be something
you *point at*, not something the tool decides for you.

Success: a bare `lf <step>` loads the native agent doc (STYLE.md via the
AGENTS.md/CLAUDE.md symlink), `scratch/`, and `wave/` — nothing else. To pull in
a README or a directory's docs, you pass `--docs`.

## Current state

- `gather_repo_root_docs` (prompt.rs:930) globs **every root `*.md`** and renders
  it into a dedicated `<lf:docs>` section. No relevance filter.
- `DocumentSource::RepoDoc` bundles **both** the root glob **and** `gather_scratch_docs`.
- `DocumentSource::Wave` / `WaveMemory` auto-added when a wave is in scope
  (prompt.rs:184).
- `DocumentSource::Area` auto-added whenever `--area` is set (prompt.rs:188);
  `gather_area_docs` pulls `*.md` from the area's ancestors + descendants.
- `--area` also flows to the step as a scope *string* (run.rs:130). It does **not**
  narrow the diff. `hash_areas` (git.rs) is wave-trigger change-detection — a
  separate system, out of scope here.
- `lfdocs: bool` (config default `true`) gates `RepoDoc`. Flag surface:
  `--lfdocs` / `--no-lfdocs`, `lfdocs_setting()`, threaded through `lf-prompt`,
  `run.rs`, `launch.rs`, and `lf op` context copy.
- Not a DTO / wire field. Rust-only.

## Target design

### Prompt sections

| Section | Behavior |
|---------|----------|
| Native agent doc (STYLE.md) | Ambient, unchanged. Loaded via AGENT_NATIVE_FILES symlink follow. |
| `<lf:scratch>` | **First-class, ambient.** Always loaded. |
| `<lf:wave>` + memory | **First-class, ambient.** Always loaded when a wave is in scope. |
| `<lf:docs>` | **Deleted.** No automatic repo-doc section. |
| `--docs` content | Rendered as ordinary included files (like `--file` / diff files). No special labeled block, no magic. |

A top-level README stops being special. It's just a path: `--docs README.md`,
`--docs '*.md'`, `--docs docs/`.

### The flag

`--docs <path>[,<path>...]` — path-delimited. Each entry is:

- a **file** → include it,
- a **glob** → include matches (e.g. `*.md`),
- a **directory** → the old `gather_area_docs` walk (ancestors + descendants `*.md`).

Default: **empty** (prefetch nothing). Config: `docs: [ ... ]` for a repo/user
default; flows set it where the workflow needs it.

### `--area` collapses into `--docs` (decision B)

`--area` is removed as a CLI flag. Its two jobs:

1. **Doc loading** → subsumed by `--docs <dir>`.
2. **Scope string passed to the step** → dropped. Not replaced. Rationale: when
   `--docs` is non-empty, that content is literally in the agent's context, so the
   agent knows what it's working on without a separate scope signal. Aligns with
   the direction of removing `area` from the wave model entirely
   (goal-driven waves, `d6e54f05`).

### LOOPFLOW.md is ambient again (default-on)

Reverse `56f2cc79`'s opt-in default and restore the always-injected operating
doc, under its original name.

- Rename builtin `OPERATE.md` → `LOOPFLOW.md`; const `OPERATE_DOC` → `LOOPFLOW_DOC`;
  section `<lf:operate>` → `<lf:loopflow>`.
- **Default included.** The meaningful switch flips: `--operate` (opt-in) becomes
  `--no-loopflow` (opt-out); internal `operate: bool` default `true`.
- **Single source.** STYLE.md's "Working in Loopflow" section is the same content
  (worktrees, `lf op`, surfaces, where-to-write). With LOOPFLOW.md ambient again,
  that section double-injects. STYLE.md drops it; LOOPFLOW.md owns it. (Its own
  note — "it now lives here in the agent doc" — is what we're reversing.)
- Wave looping-goal agents already force it on (launch.rs:196); unchanged.

## Delete (remove entirely — no compat shims)

- `lfdocs: bool` config field + default; `--lfdocs` / `--no-lfdocs` flags and `lfdocs_setting()`.
- `--area` / `-a` CLI flag, and the `area` scope string threaded to the step (run.rs:130).
- `config.area` (`area: Option<String>`) — its only effect was doc-loading + the scope string, both gone. Confirm no other consumer, then remove.
- `--operate` opt-in flag and the `operate: false` default (flips to default-on).
- `gather_repo_root_docs` and the `<lf:docs>` section render.
- The `DocumentSource::Area` auto-add (prompt.rs:188). Keep `gather_area_docs`'s walk logic — it's reused when `--docs <dir>` points at a directory — but it is no longer an automatic path.

`OPERATE.md` / `OPERATE_DOC` / `<lf:operate>` are **renamed**, not deleted (→ `LOOPFLOW.md` / `LOOPFLOW_DOC` / `<lf:loopflow>`).

## Files to change (Rust-only)

- `engine/prompt.rs` — split root-doc glob out of `RepoDoc` so `scratch/` stays
  ambient and the root glob dies; delete the `<lf:docs>` section render; route
  `--docs` targets (file / glob / dir) into the included-files path; drop the
  `Area` auto-add and `area` scope plumbing.
- `engine/config.rs` — replace `lfdocs: bool` with `docs: Vec<...>`; remove the
  `area: Option<String>` default doc field.
- `engine/launch.rs` — replace `lfdocs` override resolution with `docs`.
- `bin/lf-prompt.rs`, `bin/lf.rs` (arg-reorder list), `lf/mod.rs`,
  `lf/commands/run.rs` — swap `--lfdocs`/`--no-lfdocs`/`--area` for `--docs`.
- `lf/commands/ops/mod.rs` — `lf op` context copy uses `--docs` semantics.
- `engine/builtins.rs` + `builtins/OPERATE.md` — rename to `LOOPFLOW.md` /
  `LOOPFLOW_DOC`; `engine/prompt.rs` section render `<lf:operate>` → `<lf:loopflow>`;
  flip default to on with a `--no-loopflow` opt-out.
- `STYLE.md` — delete the "Working in Loopflow" section (moves to LOOPFLOW.md).
- Goldens: regenerate; `<lf:docs>` blocks vanish from default prompts and
  `<lf:loopflow>` now appears by default.
  `uv run python tests/goldens/update_goldens.py`.

## Migration

- `.lf/config.yaml`: `area: <dir>` → `docs: [<dir>]`. No back-compat shim (internal config).
- `lfdocs: true` had no direct successor key — its two behaviors split: `scratch/`
  is now unconditional ambient, and to keep the root-`*.md` dump add `docs: ['*.md']`.
  Most configs can just drop `lfdocs` (root-doc dump was rarely the point).
- Flows that relied on area docs set `docs:` explicitly.
- `scratch/` + `wave/` need no change — they stay ambient by construction.

## Decisions (resolved)

- `docs` default is **empty** everywhere. STYLE.md is already native; nothing to seed.
- Audit builtin flows for any that leaned on area docs; add an explicit `docs:` only
  where a flow visibly breaks without it. Note leftovers in `scratch/questions.md`.
- `lf op cp`: paths given → prefetch just those; no paths → the default ambient set
  (agent doc + LOOPFLOW.md + scratch/ + wave/).

## Done when

- Bare `lf gate` prompt contains `<lf:loopflow>`, `<lf:scratch>`, `<lf:wave>`, and the
  agent doc — and **no** `<lf:docs>`, no root `*.md` dump.
- `lf gate --docs README.md,swift/` prefetches those (file + directory walk) as
  ordinary included content; `<lf:docs>` does not reappear.
- `lf gate --no-loopflow` drops `<lf:loopflow>`.
- `--area`, `--lfdocs`, `--no-lfdocs`, `--operate` are gone (clap errors on them).
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test -p loopflow` pass.
- Goldens regenerated (`uv run python tests/goldens/update_goldens.py`) and the diff
  reviewed — `<lf:docs>` gone, `<lf:loopflow>` present by default.
