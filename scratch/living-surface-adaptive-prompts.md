# Phase 03: Surface-adaptive prompts

## Problem

Same step, different environments. `lf implement` runs identically whether launched from a headless wave executor, an interactive CLI session, Concerto on iPad, or a future TUI. But the agent's *behavior* should adapt — concise output on mobile, verbose on CLI, questions to scratch/ in auto mode, questions to the user interactively.

Today, the only environmental signal in the prompt is `run_mode` — a string "auto" or "interactive" that produces 2 lines of behavioral instruction. This handles the interaction pattern but says nothing about the rendering environment. A session from Concerto gets the same instructions as a CLI interactive session, even though one renders on a phone and the other fills a terminal.

## Approach

Add `surface` as a new dimension alongside `run_mode`. They're orthogonal:

- **run_mode** = interaction pattern: `auto` (autonomous, no user) or `interactive` (conversational, user present)
- **surface** = rendering environment: `cli`, `session`, `tui`, `mobile`

The matrix:

| Surface | Typical run_mode | Example |
|---------|-----------------|---------|
| `cli` | auto or interactive | `lf implement` or `lf design` |
| `session` | interactive | lfd session API, Concerto desktop |
| `tui` | interactive | future TUI client |
| `mobile` | interactive or auto | Concerto on iPhone/iPad |

### Surface enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Surface {
    #[default]
    Cli,
    Session,
    Tui,
    Mobile,
}
```

Serde representation: `"cli"`, `"session"`, `"tui"`, `"mobile"`. Unknown values → `Cli` (safe default, most permissive).

### How surface flows through assembly

Same path as `run_mode`:

1. **CLI** (`lf/commands/run.rs`): Always `Surface::Cli`. Auto-detected, no flag needed.
2. **Sessions** (`lfd/sessions/mod.rs`): Read from `SessionConfig.surface`. Defaults to `Surface::Session`.
3. **LaunchPromptInput**: New `surface: Surface` field (defaults to `Cli`).
4. **GatherContextOpts**: New `surface: Surface` field.
5. **PromptComponents**: New `surface: Surface` field.
6. **`format_reference_sections()`**: Combines run_mode + surface into a single behavioral block.

### What gets injected

Surface instructions append to the existing run_mode text. No new XML tags — just richer plain text in the same location.

**cli** (default): No surface-specific text. The run_mode instruction is sufficient.

**session**: "Surface: session. Output renders in a UI, streamed in real time. Keep responses scannable — prefer lists and short paragraphs over walls of text."

**tui**: "Surface: TUI. Display is constrained. Prefer concise output — summaries over verbose explanations, relevant lines over full files."

**mobile**: "Surface: mobile. Screen real estate is limited. Be concise — bullets over paragraphs, short snippets over full files. Minimize back-and-forth."

Example assembled block for mobile + interactive:
```
Run mode is interactive. This is a conversation—ask questions,
propose approaches, and wait for feedback before taking major actions.

Surface: mobile. Screen real estate is limited. Be concise—bullets over
paragraphs, short snippets over full files. Minimize back-and-forth.
```

Example for mobile + auto:
```
Run mode is auto (headless). Proceed without pausing for questions.
If you need clarification, make the best assumption you can and append
any open questions to `scratch/questions.md`.

Surface: mobile. Screen real estate is limited. Be concise—bullets over
paragraphs, short snippets over full files. Minimize back-and-forth.
```

### LOOPFLOW.md update

Add a "Surfaces" section to the builtin `LOOPFLOW.md` after the existing "Run Modes" section:

```markdown
## Surfaces

Check the surface context if present. It describes your rendering environment.

**cli** (default): Standard terminal. Full output is fine.

**session**: UI-rendered, streamed in real time. Structured, scannable output.

**tui**: Constrained terminal display. Concise output.

**mobile**: Limited screen. Very concise—bullets, short snippets, minimal back-and-forth.

Surface adapts your output style. Run mode adapts your interaction pattern.
Both apply simultaneously.
```

### SessionConfig change

Add optional `surface` field:

```rust
pub struct SessionConfig {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,  // "cli", "session", "tui", "mobile"
}
```

Concerto sends `surface: "mobile"` when creating sessions. Desktop Concerto sends `surface: "session"` (or omits it for the default). The session manager reads it and passes it through to `LaunchPromptInput`.

### Auto-detection

No configuration needed:
- `lf` CLI → `Surface::Cli` always
- `lfd` sessions → `Surface::Session` default, overridable via `SessionConfig.surface`
- Concerto → passes `"mobile"` for iOS, `"session"` for macOS

Cold start works because defaults are sensible. No `.lf/config.yaml` surface key. No environment variables. The caller knows what surface it is.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Replace run_mode with richer Surface enum | Conflates interaction pattern with environment — a mobile session can be auto or interactive | Orthogonal concerns need orthogonal fields |
| Surface-specific step variants in frontmatter | Step authors write `if mobile then...` | Violates core requirement: adaptation is in assembly, not in steps |
| Embed surface in LOOPFLOW.md conditionally | Would require templating in the builtin doc | Plain text injection is simpler and matches run_mode pattern |

## Key decisions

**Surface is separate from run_mode.** They compose. A wave executor running through Concerto is surface=mobile, run_mode=auto. An interactive CLI session is surface=cli, run_mode=interactive. This matrix is essential — collapsing it into a single dimension loses expressiveness.

**No new XML tags.** Surface instructions are plain text appended to the run_mode block. This keeps the prompt structure unchanged and avoids adding parsing surface to token budgets.

**Surface doesn't affect token budget or context gathering.** Same docs, same budget, same trimming priority regardless of surface. The only thing that changes is the behavioral instruction text. Future phases could adjust budgets per surface, but that's premature — the prompt instruction alone is enough to change agent behavior.

**`cli` produces no surface text.** CLI is the default, the most common surface, and the least constrained. Adding surface text to every CLI run would be noise. Only non-default surfaces inject additional instructions.

**Unknown surface values degrade to `cli`.** Forward-compatible. If Concerto sends `surface: "watch"` before the Rust binary knows about watches, it degrades gracefully to the default (no surface-specific instructions).

## Scope

In scope:
- `Surface` enum in `engine/prompt.rs`
- Thread surface through `GatherContextOpts` → `PromptComponents` → `LaunchPromptInput`
- Surface-aware text injection in `format_reference_sections()`
- Surface section in `LOOPFLOW.md` builtin doc
- `surface` field on `SessionConfig`
- Session manager reads surface from config, passes to `prepare_launch_prompt`
- Tests: golden prompt tests verify surface text injection for each variant
- Prompt parity test update

Out of scope:
- Surface-specific token budgets (future phase)
- Surface-specific context trimming strategies
- CLI flag for surface override (`--surface`)
- TUI client implementation
- Concerto changes to pass surface (Concerto team owns that)

## Done when

- `cargo test --all` passes with new golden prompt tests covering cli/session/tui/mobile surfaces
- `uv run pytest tests/parity/test_prompt_parity.py` passes
- A prompt assembled with `surface=mobile, run_mode=auto` contains both the headless instruction and the mobile surface instruction
- A prompt assembled with `surface=cli, run_mode=interactive` contains only the interactive instruction (no surface text)
- `SessionConfig` accepts `surface` field and it flows through to prompt assembly
- No step files modified — surface adaptation is entirely in the assembly pipeline

**Wave goal advanced:** "Same wave step works across all surfaces (headless, session, TUI, mobile)" — this phase makes the assembly pipeline surface-aware, so steps adapt without surface-specific logic.
