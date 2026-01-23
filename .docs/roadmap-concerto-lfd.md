# Roadmap: Concerto ↔ lfd Integration

Fixing the connection between Concerto (Swift) and lfd (Python daemon), then rebuilding the architecture for a flicker-free, responsive UI.

---

## The Problem

Concerto's worktree UI flickers and lags. Root causes:

1. **Protocol mismatch** — Concerto subscribes to `session.*` events, lfd emits `step_run.*`
2. **Pull-based architecture** — Every UI update requires full re-fetch from CLI
3. **Multi-pass rendering** — Status, staleness, and CI load at different times
4. **Too much UI chrome** — 6 hover buttons compete for attention

---

## Projects

| # | Project | Status | Scope |
|---|---------|--------|-------|
| 1 | [Protocol Alignment](./concerto-lfd-01-protocol.md) | **Complete** | Fix event names and field mappings |
| 2 | [Worktree State Service](./concerto-lfd-02-worktree-state.md) | **Next** | Move status calculation into lfd |
| 3 | [Push-Based Events](./concerto-lfd-03-push-events.md) | Future | Rich events with full worktree status |
| 4 | [Atomic UI Updates](./concerto-lfd-04-atomic-updates.md) | Future | Single-pass rendering in Concerto |
| 5 | [Simplified WorktreeRow](./concerto-lfd-05-worktree-row.md) | Future | Remove hover buttons, better summary |

---

## Target Architecture

```
                              lfd (daemon)
                                   │
                    ┌──────────────┼──────────────┐
                    │              │              │
              Worktree State   Session State   Job State
              (git status,     (step runs,    (triggers,
               staleness,       output)        schedule)
               CI polling)
                    │              │              │
                    └──────────────┼──────────────┘
                                   │
                         Unix Socket (push events)
                                   │
                              Concerto (UI)
                                   │
                    ┌──────────────┼──────────────┐
                    │              │              │
              WorktreeList    SessionView    JobsView
              (render only)   (render only)  (render only)
```

**Key principle:** lfd owns all state. Concerto renders what lfd tells it.

---

## Current vs Target

| Aspect | Current | Target |
|--------|---------|--------|
| Worktree status | Concerto calls `wt list`, then enriches | lfd maintains, pushes changes |
| Staleness | Concerto runs 4+ git commands/worktree | lfd calculates, includes in status |
| CI status | Concerto polls `gh pr checks` sequentially | lfd polls in background, pushes updates |
| Session events | Broken (namespace mismatch) | Working (aligned protocol) |
| UI updates | Multi-pass (flicker) | Atomic (no flicker) |
| Hover buttons | 6 buttons on every row | Context menu or 0-1 primary action |

---

## Sequencing

```
[1] Protocol Alignment ──────► Connection works, events flow
         │
         ▼
[2] Worktree State Service ──► lfd calculates status, Concerto pulls
         │
         ▼
[3] Push-Based Events ───────► lfd pushes status changes
         │
         ▼
[4] Atomic UI Updates ───────► Concerto renders atomically
         │
         ▼
[5] Simplified WorktreeRow ──► Clean, focused UI
```

Projects 2-3 can partially overlap. Projects 4-5 depend on 2-3.

---

## Success Criteria

- [ ] Concerto connects to lfd and receives events
- [ ] Worktree list loads without flicker
- [ ] Status updates appear within 1 second of git operations
- [ ] CI status appears without sequential delay
- [ ] WorktreeRow shows essential info at a glance
- [ ] No hover button overload

---

*January 2026*
