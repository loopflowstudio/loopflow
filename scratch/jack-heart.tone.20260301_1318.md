# heart.tone — loopflow voice and self-improvement

## What to build

Three things that share one pattern: loopflow agents that know themselves and get better over time.

1. **VOICE.md** — a builtin doc defining loopflow's default output voice. Anti-patterns to avoid, light flavor to add, room for the underlying model's personality. Loaded into every session.

2. **Adaptation behavior** — agents that update `.lf/` as they work. When a step hits repo-specific friction, copy it to `.lf/steps/` and adapt. When the user expresses a voice preference, update `.lf/voice.md`. These changes appear in commits — transparent, reviewable, revertable.

3. **Config self-awareness** — agents that know what's configurable and where. When an agent discovers a better default, it writes to `.lf/config.yaml`.

## User quotes

> "I want there to be a somewhat distinctive tone to loopflow"

> "Fun, but productive"

> "No opinions or feelings, more presentations and analysis... but still with a good bedside manner and chill vibes"

> "We should add some loopflow flavor, but not so much that it drowns out the underlying differences from the model"

> "I want to make sure that the system is recursively self-improving and living and updating"

> "We should encourage the system to update repo-specific versions of key steps"

> "Ideally this is super transparent and automatic to the user"

> "We want people's loopflow tone to evolve over time"

> "I'm not scared of this stuff. Let's be bold and we dial it back"

## Design

### VOICE.md

A new builtin doc, loaded like LOOPFLOW.md and RLM.md. Injected into every session's prompt as `<lf:voice>`.

**Resolution order** (only one is loaded — first match wins):
1. `~/.lf/voice.md` — user-level (applies to all repos)
2. `.lf/voice.md` — repo-level (team/project voice)
3. Builtin VOICE.md — loopflow default

"Only read one" — no merging. First match wins.

**Content of the builtin default** — see VOICE.md draft below.

Target: ~200 words for the builtin default.

### Adaptation section in LOOPFLOW.md

A new section in the system prompt that activates the existing `.lf/` override infrastructure:

```markdown
## Adaptation

Loopflow adapts to each repo through use. When you learn something repo-specific, write it down in `.lf/`.

**Steps**: When a builtin step doesn't fit this repo, copy it to `.lf/steps/<name>.md` and adapt. Changes to `.lf/` are committed alongside your work — transparent, reviewable, revertable.

**Voice**: When the user expresses a communication preference, update `.lf/voice.md`.

**Config**: When a setting should be different, update `.lf/config.yaml`. Configurable: agent, direction, area, context, exclude, lint, test, budgets.

Resolution order for everything: repo `.lf/` > user `~/.lf/` > builtin. First match wins.
```

### Step-level adaptation hooks

The system prompt plants the seed. Key steps practice it. Each gets 2-3 sentences woven into its natural conclusion:

**gate.md** — learns repo-specific quality checks:
> After completing your primary work, assess: did you discover a quality check this repo always needs? If `.lf/steps/gate.md` doesn't exist, copy the builtin and add it. If it exists, update it. Commit alongside your work.

**release.md** — learns release process:
> If you discovered repo-specific release conventions (changelog format, tag scheme, deploy hooks), update `.lf/steps/release.md`.

**land.md** — learns merge strategy:
> If you discovered repo-specific landing conventions (merge strategy, branch cleanup, CI wait behavior), update `.lf/steps/land.md`.

**ci-fix.md** — learns recurring CI patterns, with 5whys thinking:
> After fixing the immediate failure, ask: why did this get to CI? Could an earlier step have caught it? If the answer points to a missing check in gate, a missing convention in repo docs, or a recurring pattern in ci-fix itself — make that update. The fix is the symptom; the step/doc update prevents recurrence.

**implement.md** — documents repo conventions it discovers:
> If you had to discover a convention that wasn't documented (error handling pattern, test structure, naming, import style), add it to the repo's style guide (CLAUDE.md, STYLE.md) so the next session doesn't have to rediscover it. This isn't a step update — it's a repo docs update.

**review.md** — the most powerful adaptation agent. Sees the full chain:
> Review sits downstream of implement, gate, and design. When something is wrong, ask: which upstream step should have caught or prevented this? Update that step's `.lf/steps/` copy, or update repo docs if the issue was missing context. Also update `.lf/steps/review.md` itself when you notice recurring review patterns the team cares about.

**design.md** — lighter touch, updates context:
> If the design session keeps rediscovering the same context (architecture constraints, API boundaries, team preferences), update repo docs or wave context so future design sessions start with it.

### Chain of responsibility

```
gate       → updates itself (quality checks for this repo)
release    → updates itself (release process, changelog, tag scheme, deploy hooks)
land       → updates itself (merge strategy, branch cleanup, CI wait)
ci-fix     → updates gate + itself (5whys: what should have caught this earlier?)
implement  → updates repo docs (conventions it had to discover)
review     → updates implement, gate, release, land, repo docs (what should upstream have known?)
design     → updates repo docs, wave context (what context was missing?)
```

### Config self-awareness

Embedded in the adaptation section of LOOPFLOW.md. The agent needs to know:

- `.lf/config.yaml` fields: agent, direction, area, context, exclude, lint, test, budgets, interactive, push, pr, yolo
- `.lf/steps/` overrides builtin steps (per-step, within builtin flows)
- `.lf/directions/` overrides builtin directions
- `.lf/voice.md` overrides builtin voice
- `~/.lf/` for user-global versions of all the above
- Resolution: repo > user-global > builtin. First match wins. No merging (except additive config lists).

### Step voice notes

Interactive steps get a one-line reference to VOICE.md woven into existing voice guidance:

- **design.md** — "Follow VOICE.md. Let the idea drive the energy, not performed enthusiasm."
- **explore.md** — "Follow VOICE.md. Answer directly. Don't narrate your process."
- **review.md** — "Follow VOICE.md. Observations, not opinions."
- **refine.md** — "Follow VOICE.md for your own communication. Match the user's voice for their content."

### Rust changes

**`engine/prompt.rs`** — new `voice_doc` field in `PromptComponents`. Load with resolution: `~/.lf/voice.md` > `.lf/voice.md` > builtin.

**`engine/builtins/`** — new `VOICE.md` file, compiled in like `LOOPFLOW.md`.

**`format_reference_sections`** — inject `<lf:voice>` after `<lf:loopflow>` and before surface instructions.

**`LOOPFLOW.md`** — add adaptation section.

## Constraints

- VOICE.md must stay under 300 words. Every word costs context tokens in every session.
- The adaptation section in LOOPFLOW.md should be under 150 words.
- "Only read one" voice doc — no merging. First match wins.
- Don't rewrite existing step voice guidance. Add a one-line reference to VOICE.md.

## Done when

1. `cargo test --all` passes — golden prompt tests updated
2. A session with no `.lf/voice.md` gets the builtin default in its prompt
3. A session with `.lf/voice.md` gets the repo version instead
4. A session with `~/.lf/voice.md` gets the user version instead
5. LOOPFLOW.md contains the adaptation section
6. Interactive steps reference VOICE.md
7. Key steps (gate, release, land, ci-fix, implement, review, design) have adaptation hooks
8. Manual test: run `lf explore` and verify voice doc appears in `.lf/log/` prompt
