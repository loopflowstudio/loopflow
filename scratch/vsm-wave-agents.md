# vsm-wave-agents

## What to build

Ship the five Viable System Model systems as **builtin goals** — short generic
charters a looping agent can run as `lf goal s1` through `lf goal s5`.

This promotes VSM from *flows* (today's `govern-*` `scan→assess→mutate`
pipelines) to *goal-driven wave agents*. The flows survive as each system's
*hand*; the charter is its *head*.

Jack's framing (verbatim): *"for this level what is the best short-enough
GOAL.md to write"* and *"these need to be more like generically
self-organizing."*

## The register (the hard part, now settled)

A VSM charter is **not** operational (that's the flow's job), **not** an
identity ("you are the cohesion"), and **not** product-specific. It is a
self-correcting compass for a looping agent — five moves:

1. **True north** — the invariant, one line the agent steers by.
2. **The drive** — the target state, concrete enough to picture.
3. **Progress test** — "closer when X, further when Y." Self-evaluation, no dashboard.
4. **Reorientation** — the drift failure mode, and the pull back to north.
5. **Deferral** — what belongs to the sibling systems.

The deferrals form a closed ring — S1→(S2,S3), S2→(S1,S3), S3→(S4,S5),
S4→(S3,S5), S5→(S3,S4). No system reaches past its neighbors. That closure *is*
the viable-system property, in five sentences instead of five scan scripts.

Generic ⇒ the same five apply to any chord, so they ship as builtins, not
per-wave files. Recursion is free: point S3 at the root chord or a sub-chord,
same charter.

## Data structures

No new types. `Goal { prompt }` already exists. The five charters are builtin
goal `.md` files (body only, no frontmatter — matches `build/goal/ship-roadmap.md`).
`build.rs` already scans `builtins/*/goal/` and registers core-category
(`govern`) goals under flat names.

## Key functions

- **Add** `rust/loopflow/src/engine/builtins/govern/goal/{s1,s2,s3,s4,s5}.md`
  — the five charters below. They register as builtin goals through `build.rs`.
- **Reuse `lf goal` as-is** (`rust/loopflow/src/lf/commands/goal.rs`):
  `resolve_wave_name` accepts explicit names, and `load_goal` already falls back
  to builtins. `lf goal s3 --once` therefore renders the builtin S3 charter
  without a VSM-specific flag or mapping table.

## Constraints

- **Charters ship verbatim.** The wording below is the deliverable — the design
  work *was* the wording. Don't paraphrase or "improve" it during implementation.
- **Body-only builtin goals.** The builtin loader uses file content as the
  prompt without stripping frontmatter (unlike the wave-file branch). So no
  `---` frontmatter in these files, or it leaks into the prompt.
- **Don't touch the `govern-*` flows.** They remain the hands. This commit only
  adds goals + one flag.

## The five charters (ship verbatim)

### s1 → govern/goal/s1.md
```
True north: the work of the system actually gets done, in real contact with the world.

Drive toward units that are each a living whole — doing genuine work in their own
territory, as autonomous as they can bear, leaning on the center only for what they
truly can't do alone.

You are closer when a unit delivers something its environment accepts on its own
terms; further when it produces motion without contact, or hands its real work
upward. When a unit can no longer act without the center, push autonomy back down
to it.

Friction between units belongs to s2; making them add up belongs to s3.
```

### s2 → govern/goal/s2.md
```
True north: the units act in parallel without colliding.

Drive toward shared boundaries, resources, and rhythms so smooth that conflict
dissolves before it's felt — one hand, many fingers.

You are closer when parallel work proceeds without stepping on itself and clashes
resolve themselves; further when the same collision keeps recurring, or when
coordination hardens into a bottleneck everything waits on. When you catch yourself
deciding what the units should do rather than smoothing how they run together,
you've taken s3's chair — drop back to damping the oscillation.

What each unit should pursue is its own (s1); whether the whole gains is s3's.
```

### s3 → govern/goal/s3.md
```
True north: the whole is worth more than the sum of its parts.

Drive toward a system whose units reinforce each other — shared capacity flowing
wherever it does the most good, no part starving while another sits idle.

You are closer when the whole produces more than its parts could alone on the
same resources; further when a unit is optimizing itself at the system's
expense. Read each cycle against that: did the whole gain, or did you just shuffle
effort between units? When you can't tell whether a move helped the whole, you've
drifted into local optimization — return to the whole.

The future belongs to s4; identity belongs to s5. Yours is the living whole.
```

### s4 → govern/goal/s4.md
```
True north: the system stays fit for a world that keeps changing.

Drive toward an outside sensed early and a future rehearsed, so that change arrives
already accounted for — the system adapting before it is forced to.

You are closer when an external shift was seen and prepared for before it bit;
further when the world moves and the system is caught surprised, or when you chase
signals that never touch it. When your scanning stops connecting to anything the
system could act on, you've wandered into noise — return to: what here must change
because the world did?

The living now belongs to s3; whether to actually change what the system is
belongs to s5.
```

### s5 → govern/goal/s5.md
```
True north: the system stays unmistakably itself as it grows.

Drive toward one coherent identity that holds the tension between what the system is
now and what it is becoming — a whole that knows what it is and what it refuses to
become.

You are closer when growth and adaptation still read as the same system; further
when the parts pull it into something it wouldn't recognize, or when identity
ossifies and can't absorb what s4 has learned. When you find yourself arbitrating a
day-to-day tradeoff, you've dropped into s3 — rise back to: is this still us?

The present balance is s3's to run; the outside is s4's to read. You set only what
must not change.
```

## Done when

```
cargo build && cargo test -p loopflow
lf goal s3 --once
```

- The five charter files exist under `builtins/govern/goal/` and are registered
  (a `resolve_builtin_goal("s3")` unit test passes).
- `lf goal s3 --once` renders the **S3 charter** inside
  `<lf:loopflow-operating-prompt>` + `<lf:goal-context>` and stops after one
  iteration.
- `lf goal root` still loads root's own goal — unchanged.

## Deferred (not this commit)

- **gstack** — keep dormant or cut (~38 steps, 3 flows, Python converter,
  cleanly namespaced). Decision pending; its own small PR either way.
- **Flow/step simplification** — prune `build`/`ops` to what still makes sense in
  the wave-agent world. Larger; likely its own wave.
- **Symmetric five vs asymmetric governor** — we ship all five charters
  symmetric; whether S1/S2 earn standing loops or collapse to member-waves /
  reflex is a question the running system can answer, not a pre-judgment.
