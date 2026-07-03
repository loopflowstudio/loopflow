# vsm-wave-agents

Shipped: the five VSM system charters as body-only builtin goals under
`rust/loopflow/src/engine/builtins/govern/goal/{s1,s2,s3,s4,s5}.md`, auto-registered
by `build.rs` and resolved through the generic `lf goal` loader (no `--system` flag).

Forward-looking work is folded into the goals wave:
- Standing-loop wiring + symmetric-vs-asymmetric decision → `wave/goals/4-vsm-standing-loops.md`
- Flow/step pruning + gstack keep-or-cut → `wave/goals/3-prune-flow-vocabulary.md`

## Validate

```bash
cargo build && cargo test -p loopflow
lf goal s3 --once
```

- The five charter files exist under `builtins/govern/goal/` and register as
  builtin goals (`resolve_builtin_goal("s3")` unit test passes).
- `lf goal s3 --once` renders the **S3 charter** inside
  `<lf:loopflow-operating-prompt>` + `<lf:goal-context>` and stops after one iteration.
- `lf goal root` still loads root's own goal — unchanged.
