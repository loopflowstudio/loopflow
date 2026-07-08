# Worktree machinery

The stack primitives hold under real use.

## KRs

- Stacking persists child base SHA at creation (Linear eff-item).
- Stacked re-parent onto main when the parent merges is verified live (#836
  follow-through).
- `lf op next` works from the wave home (main held by the canonical checkout
  — today it errors; see 2026-07-08 session).
- Wave "home" question answered: local vs mac-mini, cross-home visibility in
  Concerto (Linear item).
