# LOO-312 assumptions and outstanding evidence

- 2026-10-22 assumptions (reversible): `ActiveInstall.activation` is an
  optional field so released lf's `active.json` still reads; a released lf
  writing the record drops it and updates fall back to `/Applications`.
  Codex's installer runs with `/bin/sh`, Claude's with `/bin/bash` (per their
  docs' `| sh` / `| bash`); not exercised against the live scripts.
  `activate-app` writes the zsh block whenever activation succeeds, even if the
  user's own config already adds the directory (the block is conditional).
  Every launch re-runs activate-app's signature check; the "already
  installed" path then returns without restarting. Measure its cost live.
- When the opened bundle is also the target, published promotion quiesces the
  running app (the caller) and reopens it. The coordinator survives because it
  re-execs retained bytes; GUI-parent exit, stdout-pipe closure and reopen are
  unproven and need the disposable-account trial.
- First-install cancellation has no prior installation to authorize. Ordinary
  helper startup stays unavailable until cancellation completes; install
  commands bypass that. Do not authorize an unadvanced candidate's startup to
  make a live proof pass.
- The no-rendering constraint prevents the live baseline required before UI
  edits. Do not substitute fixtures for the baseline or clean-account trial.
  HOME-isolated probes do not isolate machine install state, Keychain,
  preferences, Git or an OS user; do not promote or log in on this account to
  manufacture evidence.
- Jack wants Linear to be optional sharing, not a requirement anywhere in the
  ordinary local experience. Existing Task/planning coupling needs product
  work; determine the boundary with LOO-316.
- Jack confirmed GitHub sign-in during initial setup: "make sure theyre signed
  in asap". Require verified sign-in before **Open project…**, reusing an
  existing connection. The `gh` installation mechanism is unresolved, with no
  package-manager prerequisite. See `account-requirements-review.md`.
- Jack prefers scoped sessions and is content to avoid init here: app actions,
  no onboarding conversation or required init. See `init-setup-review.md`.
- The Task ends at an open Loopflow with one obvious action (Open project);
  creating a project/first Task remains LOO-316.
- Jack confirmed "drag to app -> open". See `dmg-first-launch-review.md`.
- Guided setup supports Codex and Claude Code; an existing OpenCode setup keeps
  working. No provider cost, entitlement, quota or purchase is assumed.
- Default Mac zsh gets automatic PATH persistence; other shells get the
  explicit `export` line.
