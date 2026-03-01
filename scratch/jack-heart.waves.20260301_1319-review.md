# Wave Restructure — Review

## What was implemented

Consolidated 13 wave directories into 5 competency tracks: Scale, Foundation, Trust, Context, Concerto. Each wave now represents a capability that improves forever, not a feature backlog that empties.

21 sprint files migrated and renumbered. 12 old wave directories deleted. 4 sprints/waves killed (prune step, release-minor, release-patch, hosted).

## Key choices

**Competency tracks over feature backlogs.** The old structure had 13 narrow waves (auth, chords, cross-repo, infra, mobile, opsflows, prune, release-minor, release-patch, remote, sandboxes, voicecontrol, context). Many were 1-2 sprints. The new structure groups by what gets better: Scale (multi-agent coordination), Foundation (correctness), Trust (security), Context (prompt pipeline), Concerto (the app).

**Vertical UI lives with its domain.** Chord visualization goes to Scale, not Concerto. Context breakdown UI goes to Context, not Concerto. Concerto owns app-level interaction patterns only.

**Sprint ordering biases toward learning.** Each wave puts the riskiest or most foundational sprint first so later sprints benefit from contact with reality.

**Kill list.** Prune step (small chore, do when it bothers you), release-minor/patch (working cron configs, trivial to recreate), hosted (too far out).

## How it fits together

Each wave has: README (vision + goals + risks + metrics), YAML config (flow, area, direction), numbered sprint files (content preserved from originals, renumbered 01-XX). The README frames the wave as a competency track with "Not here" boundary markers.

Sprint content was preserved wholesale — no rewriting of technical designs. Only headers, numbering, and cross-references were updated.

## Risks and bottlenecks

**None significant.** This is a planning reorganization with no code changes. The Rust test that references `wave/auth/` creates synthetic test fixtures and doesn't depend on real wave files. The `docs/wave-authoring.md` example referencing `wave/infra/` lives in the main loopflow repo and uses the old name illustratively — updating it is a separate concern for that repo.

## What's not included

- Code changes (no Rust/Python/Swift touched)
- Updates to `docs/wave-authoring.md` examples (main repo concern, not this repo)
- Migration of killed items to any backlog — they're intentionally dropped
