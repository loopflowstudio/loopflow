# LOO-312 assumptions and outstanding evidence

- 2026-10-22 assumptions (reversible): `ActiveInstall.activation` is an
  optional field so released lf's `active.json` still reads; a released lf
  writing the record drops it and updates fall back to `/Applications`.
  Codex's installer runs with `/bin/sh`, Claude's with `/bin/bash` (per their
  docs' `| sh` / `| bash`); not exercised against the live scripts.
  `activate-app` writes the zsh block whenever activation succeeds, even if the
  user's own config already adds the directory (the block is conditional).
  Every launch re-runs activate-app's signature check and PATH write;
  concept review (2026-10-22) settled that a matching settled install returns
  first (design Remaining 0). Measure the remaining first-launch cost live.
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
  existing connection.
- 2026-10-22 choice (reversible): `gh` comes from GitHub's own release
  archive (`gh_<v>_macOS_<arch>.zip`, checksum-verified) into `~/.local/bin`,
  not the admin-only `.pkg`. No quarantine is applied by an lf download; the
  binary's notarization was not checked. An existing `gh` is left alone;
  nothing updates it automatically.
- 2026-10-22 assumptions (reversible): codex/claude/github sign-in without a
  remembered Chrome profile uses the default browser (Linear and managed
  logins still require one). Status reports `executable` from PATH only, so a
  fresh `~/.local/bin` install shows missing in a shell that lacks it (the app
  adds it). Claude scoping its Keychain item by `CLAUDE_CONFIG_DIR` is
  inferred from `claude_keychain_service`, not checked against a live login.
- Jack prefers scoped sessions and is content to avoid init here: app actions,
  no onboarding conversation or required init.
- The Task ends at an open Loopflow with one obvious action (Open project);
  creating a project/first Task remains LOO-316.
- Jack confirmed "drag to app -> open".
- Guided setup supports Codex and Claude Code; an existing OpenCode setup keeps
  working. No provider cost, entitlement, quota or purchase is assumed.
- Default Mac zsh gets automatic PATH persistence; other shells get the
  explicit `export` line.

- Sync onto main (v0.14.15): main bounds retention to the target's fallback plus the one it replaced, and prunes the rest. The branch had accumulated every recorded set. The merge keeps main's bound and the branch's reuse of an already-written active record. The `failed_reopen…` fixture now expects two retained sets, not three.

- 2026-10-22 choices (reversible): a development (validation-only) helper's
  `activate-app` succeeds without changes rather than refusing, so locally
  built apps open. A restored workspace window reads the registry without
  waiting for activation; only the launch window and Session resume wait. A
  first install has no restored windows; a newer copy dragged over an older
  one is quit and reopened by promotion anyway.
