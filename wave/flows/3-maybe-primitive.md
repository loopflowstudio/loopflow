# `maybe` Primitive

**Finish line:** built-in catalog has zero `xor(X, silence)` patterns; each has been rewritten as `maybe(X)`. `silence` no longer appears as a sentinel leaf.

## Why

Three of the four xors in the built-in catalog are `xor(X, silence)` — "do X, or don't." The fourth (`build`'s inner `xor(demo, code-review)`) is really two independent "do this aspect if applicable" decisions, not a genuine fork. None of them are "pick exactly one of these two real paths."

`maybe(step)` is the honest shape. `xor` stays in the language for genuine either-or branches.

## Migration

| Before | After |
|--------|-------|
| `xor(build, silence)` | `maybe(build)` |
| `xor(garden-act, silence)` | `maybe(garden-act)` |
| `xor(s1-build, silence)` | `maybe(s1-build)` |
| `xor(demo, code-review)` | `maybe(demo) → maybe(code-review)` |

After this, `xor` likely has no users in the built-in catalog. Keep the primitive; the default "conditional" pattern is `maybe`.

## Work

1. Parse `maybe` in flow YAMLs — unary wrapper, contains a step or flow.
2. Execute: router step runs before the wrapped body, decides run/skip. Same router-prompt-append pattern used for xor.
3. Render in the Flows view: the wrapped step gets a `?` glyph or dashed border. No diamond node, no sibling sub-tree for `silence`.
4. CLI breadcrumbs per step (ties into session-state work): `[maybe:demo] ran` vs `[maybe:demo] skipped because X`.
5. Rewrite the four built-in flow YAMLs. Update `flow_tests.rs` assertions.
6. Drop the `silence.md` sentinel step if it has no remaining users after migration.

## Scope

- **In**: new primitive, parser/executor/renderer support, migration of the four built-in flows, test updates.
- **Out**: auto-migrating `.lf/flows/*.yaml` in user repos — those can be rewritten by hand or left on `xor(_, silence)` indefinitely (the primitive still works).

## Risks

- The `xor(demo, code-review)` rewrite turns one router decision into two. Verify this is actually what we want — the current single router reads the diff once and picks; two routers read twice. Acceptable cost, but the router prompts must be sharp enough that duplicate work doesn't produce contradictory skips.
- If the maybe-router's skip rate is ~0% or ~100% in practice, the primitive is paying rendering cost without information value — measure after a few weeks of use.
