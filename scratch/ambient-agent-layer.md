# Ambient Agent Layer

## What to build

Standardize what `lf` injects into **every** agent prompt (cut dead weight), and
add an opt-in, detailed "operate through loopflow" prompt aimed at Wave Agents.

After this: the ambient layer is lean (surface instructions + user-selected
context), voice is repo-opt-in rather than shipped, and `lf --operate` injects a
builtin `OPERATE.md` that tells an agent to route git/worktree/GitHub through
`lf op` and to delegate work through `lf` instead of doing it inline.

> "clean up, simplify, standardize what gets injected by loopflow. what can we
> cut... + add using loopflow prompt somewhere that is fairly detailed in what
> operations to always do through loopflow and how to do it well, optimized for
> Wave Agents, but for now it can just be something that is only included with an
> extra flag." — Jack

## The cut list (subtraction)

Injection lives in `prompt.rs` `format_system_sections` (:1622) and
`format_content_sections` (:1644).

| Block | Where | Action |
|---|---|---|
| `<lf:rlm>` (`RLM_DOC`) | system, always | **delete** — file, const, injection. (Already gone in `loopflow.goal`; deleting the same file both sides is conflict-free.) |
| `<lf:voice>` (`VOICE_DOC`) | system, interactive | **delete the builtin**; keep `.lf/voice.md` / `~/.lf/voice.md` hook. Move the prose into loopflow's own `STYLE.md` (`AGENTS.md`/`CLAUDE.md` symlink it) as a `## Voice` section — loopflow keeps its voice as repo taste. |
| `surface.instructions()` | system, always | keep |
| `<lf:wave>` + inline MEMORY.md prose | content, wave runs | **trim** the ~20-line memory blurb to a tight version (nice-to-have; land core first) |
| docs / summaries / directions | content, selected | keep |

## The add (construction)

New builtin `rust/loopflow/src/engine/builtins/OPERATE.md`, injected as
`<lf:operate>` only when the `--operate` flag is set. Single source of truth —
`loopflow.goal`'s `LOOPFLOW_OPERATING_PROMPT` const (`flow.rs`) should later read
this file instead (handoff note below; not touched here).

## Data structures

```rust
// PromptComponents (prompt.rs ~315) gains one field:
pub struct PromptComponents {
    // ...existing...
    pub operate: bool,   // inject <lf:operate> OPERATE.md
}

// builtins.rs — replace RLM_DOC/VOICE_DOC with:
pub const OPERATE_DOC: &str = include_str!("builtins/OPERATE.md");
```

## Key functions

```rust
// prompt.rs
fn format_system_sections(components: &PromptComponents) -> Vec<String>
//   - remove the <lf:rlm> push
//   - keep <lf:voice> push (interactive) but it only fires when voice_doc is Some
//   - add: if components.operate { push "<lf:operate>\n{OPERATE_DOC}\n</lf:operate>" }

fn resolve_voice_doc(repo_root: &Path) -> Option<String>
//   - drop the final `Some(VOICE_DOC.to_string())` builtin fallback
//   - returns None when no repo/user voice.md exists (default = model's voice)
```

CLI: `lf` (and the flow runner in `lf/commands/run.rs`) gains `--operate`, threaded
into the `PromptComponents` build (prompt.rs ~735). Internal flag — does not cross
the lfd wire, so no DTO/fixture change. (If a wave-launched run must set it, that's
a later wire concern — note, don't build now.)

## OPERATE.md (draft content)

```markdown
# Operating Through Loopflow

You are running inside loopflow. Loopflow owns git, worktrees, delegation, and
release plumbing — route those operations through `lf`, not around it. Doing them
by hand breaks the machinery loopflow relies on (worktree naming, merge queue,
context inheritance).

## Git, worktrees, GitHub → `lf op`

Never use raw `git`/`gh` or the harness's native worktree tools for these. `lf op`
carries loopflow-specific behavior (sibling worktree convention, merge queue, wave
rotation) that hand-run commands silently corrupt.

    lf op commit -m "message" -p     # commit and push
    lf op pr --title "..."           # create/update PR
    lf op land                       # submit to merge queue
    lf op rebase                     # rebase onto main
    lf op next                       # preserve worktree, fresh branch
    lf op wt create my-feature       # sibling worktree ../<repo>.my-feature
    lf op wt switch my-feature       # cd to existing worktree
    lf op wt prune                   # clean up merged worktrees

The sibling naming (`<repo>.<name>`) is load-bearing. Worktrees created elsewhere
won't be recognized and may be corrupted during land rotation.

## Delegate — don't do the work inline

Dispatch an `lf` flow or step for real implementation work. A dispatched child
inherits full loopflow context (repo docs, style guide, area docs); inline edits
in your own session do not, and they bloat this transcript with work that belongs
in a child.

Inline edits are only for trivial fixes smaller than the cost of dispatching —
and when you do one, say why. Keep this session about decisions and coordination.

When interactive subagent sessions are available, use them to launch the work,
steer it, answer questions, and read the result back.

## Where to write

- `scratch/<branch>.md` — design doc for the current work
- `scratch/questions.md` — open questions, blockers, assumptions
- Code — the actual work

## Checkpoint and proceed

Don't ask permission for reversible work — editing files, sketching code, running
local builds or tests. Commit history is the safety net.

    # tree dirty? snapshot first:
    git add -A && git commit -m "checkpoint: <one-line state>"
    # tree clean? HEAD is the rollback point. Go.

Still ask before: pushing/force-pushing, opening/closing PRs, sending messages or
calling external APIs with side effects, and destructive ops (`rm -rf`, dropping
tables, deleting branches).

## Adaptation

When you learn something repo-specific, write it into `.lf/`: adapt a step
(`.lf/steps/<name>.md`), a direction (`.lf/directions/<name>.md`), voice
(`.lf/voice.md`), or config (`.lf/config.yaml`). Changes to `.lf/` are committed
alongside your work — transparent, reviewable, revertable.
```

## Constraints

- `OPERATE.md` is the single source. Do **not** duplicate this prose into another
  const. Handoff: `loopflow.goal` should later replace its `LOOPFLOW_OPERATING_PROMPT`
  const with a read of `OPERATE_DOC`.
- Keep `resolve_voice_doc`'s repo/user resolution intact — only the builtin
  fallback is removed. A repo with `.lf/voice.md` still gets its voice.
- `--operate` is an internal flag, not a DTO field. Don't add serde/fixture churn.
- The reversible-work / checkpoint discipline moves *into* `OPERATE.md` (it's
  operating mechanism); it leaves `VOICE.md` with the aesthetic.
- Voice prose lands in `STYLE.md` under a `## Voice` heading. Fix its dangling
  `(see LOOPFLOW.md)` reference — LOOPFLOW.md is gone; point at STYLE.md's own
  "Checkpoint and proceed" section. Then delete builtin `VOICE.md`.

## Done when

```bash
cargo test -p loopflow            # prompt tests green
```

- `cargo build` succeeds; no `RLM_DOC` / `VOICE_DOC` references remain
  (`rg 'RLM_DOC|VOICE_DOC|lf:rlm' rust/` is empty).
- Prompt with no flag and no `.lf/voice.md`: contains neither `<lf:rlm>`,
  `<lf:voice>`, nor `<lf:operate>`.
- Prompt with a repo `.lf/voice.md`: contains `<lf:voice>` (hook preserved).
- Prompt built with `operate = true`: contains `<lf:operate>` with the `lf op`
  guidance; without the flag it does not.

## Handoff to loopflow.goal

- Point `render_goal`'s operating prompt at `OPERATE_DOC` (retire the inline
  `LOOPFLOW_OPERATING_PROMPT` const) so the loop session and `--operate` share one
  prompt.
- Add the `lf op` git/worktree guidance the goal-branch prompt currently lacks —
  it comes for free once it reads `OPERATE.md`.
```
