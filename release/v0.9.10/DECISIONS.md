# Decisions — unreleased

A ledger of intent and policy decisions made during this release cycle. Each entry captures *why* something is the way it is — the kind of thing a contributor six months from now would want to cite. Not a changelog; a rationale record.

If this directory exists, `lf op release run` promotes it to `release/v<version>/` and feeds this file into the release-notes step as primary source material.

---

## 2026-04-23 — Introduce decisions log

**Context:** Release notes were generated from PR titles and merge data. Intent and policy decisions made during design and iteration weren't captured anywhere durable — they lived in chat transcripts and got lost.

**Decision:** Add `release/<version>/DECISIONS.md` as an append-only ledger that interactive runs update as decisions are made. During `lf op release run`, Loopflow promotes `release/unreleased/` to `release/v<version>/`, archives `RELEASE_NOTES.md` to `release/v<version>/NOTES.md`, and passes `DECISIONS.md` into the release-notes step as primary input.

**Implications:**
- Release notes become narrative-first instead of PR-dump-first.
- Interactive steps (design, kickoff, iterate, review-design, triage) carry an implicit responsibility to log sufficiently-important decisions. Headless runs do not — too noisy.
- Bar for "sufficiently important": something a contributor would cite six months later. Intent changes, policy choices, scope calls, paths not taken. Not "I fixed a bug."
- `release/vX.Y.Z/NOTES.md` preserves per-version notes; root `RELEASE_NOTES.md` remains the always-latest file GitHub displays.
