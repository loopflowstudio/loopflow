# Context generation: explicit docs, ambient state, measurement only

Drop `lfdocs` as a product concept and simplify the context engine around the
things users actually mean:

- `scratch/` is ambient working state.
- `wave/<name>/` and `MEMORY.md` are ambient wave state when a wave is in scope.
- `--docs` is explicit additive prefetch.
- diff and clipboard are explicit switches.
- token counts are visibility, not control.

No prompt content should silently disappear because a budget was exceeded. If
context is too large, the header and session snapshot should make that obvious;
the fix is to clean the artifact or narrow `--docs`, not have context generation
guess what matters least.

## Product intent

A bare `lf <step>` should include the operating guidance, native agent doc,
`scratch/`, and the current wave context. Repo docs are no longer automatic.
Pulling in README files, package docs, or cross-repo docs is an explicit
`--docs` choice.

`--docs foo` adds `foo`. It never removes ambient `scratch/` or wave context.

## Core model

Make the core API describe product inputs directly. `DocumentSource` should be
accounting metadata, not the language for deciding what to gather.

```rust
pub struct GatherContextOpts {
    pub repo_root: PathBuf,
    pub step: Option<String>,
    pub message: Option<String>,
    pub operate: bool,
    pub surface: Surface,
    pub directions: Vec<String>,
    pub docs: Vec<String>,
    pub files: Vec<String>,
    pub wave: Option<String>,
    pub include_diff: bool,
    pub include_diff_files: bool,
    pub include_clipboard: bool,
    pub related_repos: Vec<RelatedRepoContext>,
}
```

`GatherSpec.sources: Vec<DocumentSource>` and `default_gather_sources()` go away.
There is no longer a `RepoDoc` switch that also means “load scratch” and “maybe
load wave.”

Document sources become:

```rust
pub enum DocumentSource {
    Step,
    Direction,
    Scratch,
    Wave,
    WaveMemory,
    Docs,
    Summary,
    Diff,
    Clipboard,
}
```

Delete `RepoDoc` and `Area`. Directory docs are just `Docs`.

## Gather pipeline

`gather_context()` should read like the product:

```rust
let scratch = gather_scratch_docs(repo_root)?;
let wave_docs = gather_wave_docs(repo_root, opts.wave.as_deref())?;
let wave_memory = gather_wave_memory_doc(repo_root, opts.wave.as_deref())?;
let docs = gather_doc_targets(repo_root, &opts.docs, &opts.related_repos)?;
let files = gather_files(repo_root, &opts.files)?;
```

Stable order:

1. scratch
2. wave docs
3. wave memory
4. explicit docs
5. explicit files / diff files

`--docs` resolution:

- file: include that file
- glob: include matches
- directory: include ancestor/descendant `*.md` docs using the existing area-doc
  walk semantics
- cross-repo target: keep `repo:path` resolution against related repos

One hardcoded guardrail is enough:

```rust
const MAX_EXPLICIT_DOC_FILES: usize = 100;
```

Apply it after resolving the full `docs` list. If explicit docs resolve above
the cap, fail with a clear error asking the user to narrow `--docs`. Do not make
this configurable and do not apply it to ambient `scratch/`, wave docs, or
`MEMORY.md`.

## Measurement, not budgets

Keep token accounting everywhere. Delete trimming.

Replace:

```rust
let budgeted = trim_context_with_breakdown(gathered, DEFAULT_CONTEXT_BUDGET);
let prompt = format_prompt(PromptFormatMode::Full, &budgeted);
```

with:

```rust
let gathered = gather_context(&opts)?;
let breakdown = measure_context(&gathered);
let prompt = format_prompt(PromptFormatMode::Full, &gathered);
```

Delete:

- `BudgetedContext`
- `DEFAULT_CONTEXT_BUDGET`
- `trim_context_with_breakdown`
- drop-order tests
- `Config.budgets`
- `BudgetConfig`
- `ContextSnapshot.budget`
- CLI `% of 75k` display

Keep:

- per-source token totals
- per-source file counts
- per-document token entries
- total token count
- diff tier metadata

The CLI header should show token counts and total, but no budget percentage.
Concerto should keep its context-size UI, updated from `repo_doc` / `area` to
`docs`.

## Diff

Keep diff tiering. That is representation selection, not context budgeting:

- small branch diff: unified diff
- large branch diff: stat
- `diff_files` / explicit files: included when requested

Do not route diff behavior through `DocumentSource` input switches. Use
`include_diff` and `include_diff_files`.

## Memory

Removing budgets means `MEMORY.md` is no longer overflow context. It is durable
wave state that should be included whenever a wave is in scope.

Change the wave prompt from a token-budget framing to a maintenance invariant:

- read memory every iteration
- keep it compact enough to read every iteration
- correct stale entries
- promote stable architectural notes to wave docs or explicit docs
- delete session-specific notes

If memory grows too large, context measurement should reveal it. The wave should
maintain memory; the context engine should not silently drop it.

## Delete / rename

Delete:

- `lfdocs: bool` config field and CLI flags
- `--area` / `-a` CLI flag and run-scope string plumbing
- `config.area` for prompt context
- `DocumentSource::RepoDoc`
- `DocumentSource::Area`
- `ContextBreakdown.area_name`
- `ContextBreakdown.area_doc_count`
- area rows in CLI context output
- `budgets.area`, `budgets.docs`, `budgets.diff`
- all context trimming code

Rename:

- `RepoDoc` source key -> `Docs`
- session snapshot source `"repo_doc"` -> `"docs"`
- `gather_area_docs` -> directory docs helper, returning `DocumentSource::Docs`
- `OPERATE.md` / `<lf:operate>` -> `LOOPFLOW.md` / `<lf:loopflow>`

## Migration

`.lf/config.yaml`:

- `area: <dir>` -> `docs: [<dir>]`
- `lfdocs: true` has no direct replacement. To reproduce the old root markdown
  dump, use `docs: ['*.md']`.
- remove `budgets:` context settings

No compatibility shims. This is internal config.

## Done when

- Bare `lf gate` includes `<lf:loopflow>`, `<lf:scratch>`, wave docs/memory when
  scoped, and the native agent doc. It does not include root `*.md` docs.
- `lf gate --docs README.md,swift/` adds those docs without removing scratch or
  wave context.
- `--docs` resolving over `MAX_EXPLICIT_DOC_FILES` fails clearly.
- No `<lf:docs>` section exists.
- `--area`, `--lfdocs`, `--no-lfdocs`, and `--operate` are gone.
- Context size output still reports source tokens, file counts, document entries,
  and total tokens.
- No context source is silently trimmed after gathering.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test -p loopflow`,
  and `uv run python tests/goldens/update_goldens.py` pass.
