# Newcomer setup review handoff

## Confirmed direction

Jack chose download → drag into Applications → open. Initial setup connects a
coding tool and establishes GitHub sign-in early, before **Open project…**.
Linear is optional sharing in the intended product. Jack prefers scoped
sessions and is content to avoid an init conversation; this Task uses app
actions and does not redesign repo init.

## Design and evidence

- [Current design](a-newcomer-gets-from-download.md): complete implementation
  target, integration owners, acceptance checks, and forbidden near-misses.
- [DMG first launch](dmg-first-launch-review.md): Jack's acquisition choice,
  source findings, changed first slice, and missing activation proof.
- [Account requirements](account-requirements-review.md): early GitHub,
  optional Linear, the existing auth owner, and the `gh` dependency.
- [Init boundary](init-setup-review.md): Jack's scoped-session preference,
  existing skill behavior, and why no conversation is required here.
- [Outstanding evidence](questions.md): implementation constraints and evidence
  still needed. No live app, clean-account, login, or product test acceptance
  was performed during review. Markdown diff checks passed.

## Remaining work and next useful action

Start with a live baseline and the unregistered copied app's activation path
through the existing Rust installer. Resolve no-prior-receipt restart/retry,
durable helper registration, app destination retention, and shared PATH setup.
Then implement provider and GitHub actions, including acquiring `gh` without
requiring a package manager, and prove the clean-account download/drag/open path.

The broader removal of Linear's planning dependency needs coordination with
LOO-316; no planning records were changed or local Task support claimed here.
The product decisions above do not establish that behavior already works.

This review records feedback and design changes only. Jack completes the
review Session; the following loop-decide owns navigation. No navigation
decision is recorded here.
