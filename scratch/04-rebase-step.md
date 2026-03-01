# `lf rebase` step with fast-path

## Problem

`lf rebase` runs a full agent session for every rebase, even though most rebases are clean. The fast-path infrastructure (built in sprint 01, used by `ingest`) lets steps try a mechanical command first and only spin up an agent on failure. This sprint adds `lf rebase` as the second consumer.

## Approach

Add `fast-path: lf ops rebase` to the `rebase` step frontmatter. Revise the step prompt to focus on conflict resolution — the only scenario where the agent fires.

Two files change: `.lf/steps/rebase.md` and the builtin copy at `rust/loopflow/src/engine/builtins/steps/ops/rebase.md`.

No changes to `lf ops rebase`, the fast-path infrastructure, or any Rust code.

### How it works

```
lf rebase
  → fast-path: sh -c "lf ops rebase"
    → no conflicts → push → exit 0 → done (ops speed)
    → conflicts → ops internal agent resolves → exit 0 → done
    → conflicts → ops internal agent fails → exit non-zero
      → step agent fires with <lf:fast-path-failure> context
        → re-initiates rebase, resolves conflicts, pushes
        → if ambiguous: ask user (interactive) or scratch/questions.md (headless)
```

Three recovery layers: mechanical rebase, ops internal agent, step agent. No recursion risk — `run_builtin_agent` loads the step content as a prompt without checking fast-path.

### Step prompt design

The step prompt serves two contexts:
1. **Step agent after fast-path failure** — gets `<lf:fast-path-failure>` prepended with failure output
2. **Ops internal agent on conflicts** — runs the step content directly via `run_builtin_agent`

Both need the same workflow: understand branch intent, rebase, resolve conflicts, push. The prompt stays general enough for both but adds:
- API reference: `lf ops rebase [--onto BRANCH]`
- Ambiguous conflict handling (surface vs `scratch/questions.md`)
- Tighter conflict resolution strategy

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `--no-recover` flag on `lf ops rebase` | Step agent always handles conflicts directly | Changes ops code, loses layered recovery |
| Raw git commands as fast-path | No agent in subprocess | Reimplements branch detection, push logic |
| No changes | Simpler | Every rebase launches an agent |

## Key decisions

1. **`lf ops rebase` unchanged.** Its internal agent is the first conflict recovery layer. The step agent is the fallback.

2. **Step prompt stays general.** Both the ops internal agent and the step agent use the same content. The fast-path failure context (prepended automatically) differentiates the two scenarios for the step agent.

3. **Ambiguous conflicts surface, not guess.** Interactive → ask user. Headless → `scratch/questions.md`. Don't silently choose wrong code.

## Scope

- In scope: `.lf/steps/rebase.md` + builtin copy (`rust/.../rebase.md`)
- Out of scope: `lf ops rebase`, fast-path runner, any Rust code

## Done when

```bash
lf rebase   # no conflicts: ops speed, no agent
lf rebase   # conflicts: resolved by ops agent or step agent
lf rebase   # ambiguous conflict: agent asks user (interactive) or notes it (headless)
```
