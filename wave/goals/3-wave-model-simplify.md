---
priority: medium
---

# Simplify the wave/chord data model

**Finish line:** One self-referential `waves` table expresses both waves and
chords; the wave-structure domain uses parent/child/sibling, never "member";
cross-repo Goals work via children in different repos.

## Context

Chords once had `chords` + `chord_members` tables (migration 011), dropped
(028) and folded into `waves` via `parent_wave_id`, `wave_type`, `position`.
The structure is already self-referential; the cleanup is conceptual +
terminological. See DECISIONS.md 2026-06-30.

## What to shape

- **A chord is a wave with children** — `wave_type = chord`, children via
  `parent_wave_id`. No separate chord entity.
- **Vocabulary: parent / child / sibling.** Purge "member" from the
  wave-structure domain (code, docs, APIs). Leave VSM/govern "member" unless it
  bleeds in.
- **Cross-repo Goals** fall out for free: a cross-repo Goal is a chord whose
  children live in different repos; the parent spans whatever its children span.
  `wave.repo` stays single on each leaf.
- **Open:** whether a single *leaf* Looping Agent may span repos directly (many
  worktrees, coordinated cross-repo PRs) for tightly-coupled changes. Default:
  no — chord-spanning only — until atomicity proves necessary.

## Done when

- Chord contents query = children where `parent_wave_id = id`; no "member" in
  the wave-structure code; a two-repo chord runs with a child Looping Agent per
  repo.
