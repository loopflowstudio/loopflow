# Phase 03: Surface-adaptive prompts

## Problem

Same step, different environments. `lf implement` runs identically whether launched from a headless wave executor, an interactive CLI session, or Concerto on iPad. But the agent's *behavior* should adapt — concise output on iPhone, verbose on CLI, questions to scratch/ when headless, questions to the user interactively.

Today, the only environmental signal in the prompt is `run_mode` — a string "auto" or "interactive" that produces 2 lines of behavioral instruction. This handles the interaction pattern but says nothing about the rendering environment. A session from Concerto gets the same instructions as a CLI session, even though one renders on a phone and the other fills a terminal.

## Approach

Replace `run_mode` with `Surface`. Each surface implies its own interaction pattern — no second dimension needed.

| Surface | Interactive? | Example |
|---------|-------------|---------|
| `cli` | yes | `lf implement`, `lf design` |
| `headless` | no | wave executor, CI, lfd batch runs |
| `concerto_mac` | yes | Concerto session on macOS |
| `concerto_iphone` | yes | Concerto session on iPhone/iPad |

### Surface enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Surface {
    #[default]
    Headless,
    Cli,
    ConcertoMac,
    ConcertoIphone,
}

impl Surface {
    pub fn is_interactive(&self) -> bool {
        !matches!(self, Surface::Headless)
    }
}
```

Serde representation: `"headless"`, `"cli"`, `"concerto_mac"`, `"concerto_iphone"`. Unknown values → `Headless` (safe default — don't block waiting for a user who isn't there).

### How surface flows through assembly

Replaces `run_mode` in the existing path:

1. **CLI** (`lf/commands/run.rs`): Always `Surface::Cli`.
2. **Sessions** (`lfd/sessions/mod.rs`): Read from `SessionConfig.surface`. Defaults to `Surface::ConcertoMac`.
3. **Wave executor**: Always `Surface::Headless`.
4. **LaunchPromptInput**: `surface: Surface` replaces `run_mode: String`.
5. **GatherContextOpts**: `surface: Surface` replaces `run_mode`.
6. **PromptComponents**: `surface: Surface` replaces `run_mode`.
7. **`format_reference_sections()`**: Generates behavioral text from surface alone.

### What gets injected

Each surface produces a single behavioral block — interaction pattern + rendering guidance combined.

**cli** (default):
```
Run mode is interactive. This is a conversation—ask questions,
propose approaches, and wait for feedback before taking major actions.
```

**headless**:
```
Run mode is auto (headless). Proceed without pausing for questions.
If you need clarification, make the best assumption you can and append
any open questions to `scratch/questions.md`.

No rendering environment. Output is logged, not displayed.
Optimize for structured, parseable output over human readability.
```

**concerto_mac**:
```
Run mode is interactive. This is a conversation—ask questions,
propose approaches, and wait for feedback before taking major actions.

Surface: Concerto (macOS). Output renders in a desktop UI, streamed
in real time. Keep responses scannable—prefer lists and short paragraphs
over walls of text.
```

**concerto_iphone**:
```
Run mode is interactive. This is a conversation—ask questions,
propose approaches, and wait for feedback before taking major actions.

Surface: Concerto (iPhone). Screen real estate is limited. Be concise—bullets
over paragraphs, short snippets over full files. Minimize back-and-forth.
```

### LOOPFLOW.md update

Replace the "Run Modes" section with a "Surfaces" section:

```markdown
## Surfaces

Check the surface at the top of the prompt. It determines your interaction
pattern and output style.

**cli** (default): Interactive terminal session. Ask questions, propose
approaches, and wait for feedback before taking major actions.

**headless**: Autonomous, no user. Proceed without pausing for questions.
Make best-effort assumptions and append open questions to
`scratch/questions.md`. Output is logged, not displayed.

**concerto_mac**: Interactive desktop UI. Ask questions and wait for
feedback. Keep responses scannable—lists and short paragraphs.

**concerto_iphone**: Interactive, small screen. Ask questions and wait
for feedback. Be concise—bullets, short snippets, minimal back-and-forth.
```

### SessionConfig change

Add optional `surface` field:

```rust
pub struct SessionConfig {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,  // "cli", "headless", "concerto_mac", "concerto_iphone"
}
```

Concerto sends `surface: "concerto_iphone"` from iOS, `surface: "concerto_mac"` from macOS. The session manager reads it and passes it through to `LaunchPromptInput`.

### Auto-detection

No configuration needed:
- `lf` CLI → `Surface::Cli` always
- `lfd` sessions → `Surface::ConcertoMac` default, overridable via `SessionConfig.surface`
- Wave executor → `Surface::Headless` always
- Concerto iOS → passes `"concerto_iphone"`
- Concerto macOS → passes `"concerto_mac"`

### Migration

`run_mode` is removed from all structs and replaced by `surface`. The `is_interactive()` method provides the same boolean that code currently derives from `run_mode == "interactive"`. Callers that check `run_mode` switch to `surface.is_interactive()`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep run_mode + surface as two fields | Two dimensions, but they don't compose freely — every surface implies its run_mode | One field is simpler when there's no real independence |
| Generic surfaces (`session`, `tui`, `mobile`) | Abstracts over clients that don't exist yet | Name the real clients — `#[non_exhaustive]` handles future ones |
| Surface trait instead of enum | More extensible, but more machinery | Four variants don't need a trait; revisit if surfaces multiply |
| Surface-specific step variants in frontmatter | Step authors write `if concerto_iphone then...` | Adaptation belongs in assembly, not in steps |

## Key decisions

**Surface replaces run_mode.** Every surface implies whether it's interactive. CLI is interactive. Headless is auto. Concerto surfaces are interactive. There's no real case where the same surface is sometimes auto and sometimes interactive, so two fields is unnecessary complexity.

**No new XML tags.** Surface instructions are plain text in the same location where run_mode text lives today. No structural changes to the prompt.

**Surface doesn't affect token budget or context gathering.** Same docs, same budget, same trimming priority regardless of surface. The only thing that changes is the behavioral instruction text. Future phases could adjust budgets per surface, but that's premature.

**`Headless` is the default.** Safe degradation — if surface is unknown, assume no one's watching. Don't block waiting for a user who isn't there.

**`cli` produces the same text as today's `run_mode=interactive`.** The behavioral text is identical to what exists now — this is a transparent migration for CLI users.

**Surfaces name real clients, not abstract categories.** `ConcertoMac` and `ConcertoIphone` instead of `session` and `mobile`. `#[non_exhaustive]` means we add `ConcertoVision`, `Tui`, or anything else when it exists — not before.

**Unknown surface values degrade to `Headless`.** Forward-compatible. If Concerto sends `surface: "concerto_vision"` before the Rust binary knows about it, it degrades gracefully to headless (won't hang waiting for input).

## Scope

In scope:
- `Surface` enum in `engine/prompt.rs` with `is_interactive()` method
- Replace `run_mode` with `surface` in `GatherContextOpts`, `PromptComponents`, `LaunchPromptInput`
- Surface-aware text generation in `format_reference_sections()`
- Replace "Run Modes" with "Surfaces" in `LOOPFLOW.md` builtin doc
- `surface` field on `SessionConfig`
- Session manager reads surface from config, passes to `prepare_launch_prompt`
- Tests: golden prompt tests verify text for each surface variant
- Prompt parity test update
- Remove `run_mode` field from all structs

Out of scope:
- Surface-specific token budgets (future phase)
- Surface-specific context trimming strategies
- CLI flag for surface override (`--surface`)
- Concerto changes to pass surface (Concerto team owns that)

## Done when

- `cargo test --all` passes with golden prompt tests for all four surfaces
- `uv run pytest tests/parity/test_prompt_parity.py` passes
- A prompt assembled with `Surface::Headless` contains the auto/headless instruction
- A prompt assembled with `Surface::Cli` contains only the interactive instruction (same as today)
- A prompt assembled with `Surface::ConcertoIphone` contains both interactive and concise-output instructions
- `SessionConfig` accepts `surface` field and it flows through to prompt assembly
- `run_mode` field is gone from all structs
- No step files modified — surface adaptation is entirely in the assembly pipeline

**Wave goal advanced:** "Same wave step works across all surfaces (headless, session, TUI, mobile)" — this phase makes the assembly pipeline surface-aware, so steps adapt without surface-specific logic.
