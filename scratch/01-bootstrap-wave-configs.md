# Bootstrap Wave Configs

## Problem

The four redesign waves exist as directories on disk (`wave/chord-model/`, `wave/clear-the-deck/`, etc.) but aren't registered in lfd. There's no redesign chord-wave to coordinate them. Nothing is wired together — the foundation for tend cycles isn't there.

This blocks everything in Phase 1. Until waves are registered and the chord-wave exists, tend has nothing to observe.

## Two phases of wave existence

A wave lives in two places:

1. **Filesystem** — `wave/<name>/` with a README, work items, and YAML config. This is the wave's identity: what it's for, what it works on, how it's configured. The filesystem is the source of truth for definition.

2. **lfd registration** — a record in lfd's database with runtime state (status, iteration, run history). This is the wave's execution: is it running, where, how many cycles has it completed.

Creating the directory is defining the wave. Registering it with lfd is deploying it to a machine. A wave can exist on disk without being registered (dormant), and re-registering after an lfd restart should be straightforward because the definition lives in files, not the database.

Chord-waves follow the same pattern. A chord-wave is a wave whose area covers `wave/` — its work items are coordination, its output is wave mutations, its default flow is `tend`. No separate data model, no separate CRUD. Same `lfq` commands, same `wave/` directory structure, same YAML config.

## Approach

Two pieces, delivered as one PR:

### 1. Redesign chord-wave on disk

Create the redesign chord-wave as a wave directory:

```
wave/redesign/
  redesign.yaml
  README.md
```

```yaml
# wave/redesign/redesign.yaml
flow: tend
area:
  - wave/chord-model/
  - wave/clear-the-deck/
  - wave/agent-embedding/
  - wave/signals/
direction:
  - care
  - clarity
```

The area list IS the membership. No separate membership table — the chord-wave's area points at its member waves' directories. Concerto reads these relationships to render the hierarchical graph view.

The README describes the chord-wave's purpose: build itself, then tend its own construction. The recursive case.

### 2. Bootstrap script

`scripts/bootstrap-redesign.py` — idempotent script that registers all five waves (four member waves + the redesign chord-wave) with a running lfd:

```python
#!/usr/bin/env python3
"""Register the redesign waves with lfd."""
import loopflow.api as loopflow

REPO = "."
WAVES = [
    "chord-model",
    "clear-the-deck",
    "agent-embedding",
    "signals",
    "redesign",       # the chord-wave, same as any other
]

def main():
    for name in WAVES:
        existing = loopflow.wave(name)
        if existing:
            print(f"  {name} exists (id={existing.id})")
            continue
        w = loopflow.create_wave(name, REPO)
        print(f"  created {name} (id={w.id})")

    # Verify the chord-wave sees its members via area
    redesign = loopflow.wave("redesign")
    print(f"\nredesign wave: {redesign.id}")
    print(f"  flow: {redesign.flow}")
    print(f"  area: {redesign.area}")
    print(f"  status: {redesign.status}")

if __name__ == "__main__":
    main()
```

The script uses `loopflow.api` directly. Idempotent — `create_wave` returns 409 on duplicate name, script skips and continues.

No chord-specific API calls. No membership mutations. The chord-wave is created the same way as any other wave. Its YAML config declares the area, which is where membership lives.

## Chord infrastructure teardown

The existing chord-specific infrastructure becomes dead weight under this model:

**Rust (lfd):**
- `chords` table in SQLite
- `chord_members` table
- `/v0/chords` HTTP routes (8 endpoints)
- `ChordDto`, `CreateChordRequest`, `AddChordMemberRequest`
- Handler tests in `chords.rs`

**Python:**
- `Chord` model in `models.py`
- Chord methods in `client.py` (8 methods)
- Chord functions in `api.py` (8 functions)
- Chord tests in `test_client.py` and `test_models.py`

This teardown is significant (~500 LOC across Rust and Python) but mechanical. Two options:

**Option A: Tear down in this PR.** Clean break. No dead code on the branch. Larger PR but the removal is straightforward — delete tables, routes, client methods, tests.

**Option B: Defer teardown.** Ship the chord-wave + bootstrap, mark chord APIs as deprecated, remove in a follow-up. Smaller initial PR but carries dead code temporarily.

Recommendation: **Option A.** The chord infrastructure has no external consumers. Dead code that contradicts the new model is confusing. Rip it out while the context is fresh.

## What the system needs to recognize chord-waves

A chord-wave is just a wave, but the system should apply defaults when it detects one. Convention: if any area path starts with `wave/`, the wave is a chord-wave.

**Default flow:** `tend` (scan-waves → assess → propose → apply). The tend steps carry S2–S5 concerns as built-in behavior — coordination, optimization, intelligence, identity questions. These aren't configured per chord; they're what tend *is*.

**Default behavior in Concerto:** Chord-waves render as graph nodes with edges to their member waves (derived from area paths matching `wave/<name>/`). The hierarchical view is a Concerto concern, not an lfd concern.

This detection logic doesn't need to land in this PR — it's needed when tend actually runs (chord-model/02). For bootstrap, we just create the wave with the right config and verify it's queryable.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep separate Chord model | Two concepts for one thing | Chords are waves. Extra model adds indirection without value. |
| Derive membership at query time from filesystem | No explicit area list in YAML | Area is already the mechanism for scoping — use it. |
| Auto-detect chord-waves on lfd startup | Magic | The user decides what's a chord-wave by setting area. Convention over detection. |
| Bootstrap via CLI commands only | More moving parts, same result | Script is direct and auditable. |

## Key decisions

**Chords are waves.** Same data model, same CLI, same filesystem structure. The only distinction is area scope (over `wave/`) and default flow (`tend`).

**Membership is area.** The chord-wave's area list declares its members. No separate membership relationship. Adding a member = updating the area list. Removing a member = removing the area path.

**Filesystem is identity, lfd is runtime.** The `wave/redesign/` directory defines what the chord-wave is. Registering it with lfd deploys it. You can have waves on disk that aren't registered (planned but not running).

**Tear down chord infrastructure.** No separate chord tables, routes, or APIs. Waves are the only first-class object.

## Scope

**In scope:**
- `wave/redesign/` directory (YAML + README)
- `scripts/bootstrap-redesign.py`
- Chord infrastructure teardown (Rust routes/tables, Python client/models)
- Update `wave/chord-model/README.md` to reflect new model
- Tests: bootstrap script, verify chord-wave queryable via `lfq show`

**Out of scope:**
- Tend flow steps (chord-model/02)
- Chord-wave detection logic in lfd (needed for default flow, lands with tend)
- Letta integration (chord-model/03)
- Concerto graph view (agent-embedding)
- `lfq show` rendering member waves inline (nice-to-have, not blocking)

## Done when

- `wave/redesign/redesign.yaml` exists with area pointing at four member waves
- `uv run python scripts/bootstrap-redesign.py` registers all five waves idempotently
- `lfq show redesign` shows the chord-wave with its area list
- Chord-specific infrastructure removed (tables, routes, client methods)
- All existing tests pass (chord-specific tests deleted, wave tests unaffected)
- `wave/chord-model/README.md` updated

## Wave alignment

**Goals served:**
- "Tend flow runs against the redesign chord's own waves" — this creates the chord-wave and registers the member waves, the prerequisite for tend
- "VSM expressibility" — chords-as-waves means any nesting depth is just waves-over-waves, no special hierarchy needed

**Risks checked:**
- "Recursive bootstrapping means early tend cycles run on incomplete machinery" — mitigated by making bootstrap idempotent and independent of tend. The chord-wave exists before tend does.
