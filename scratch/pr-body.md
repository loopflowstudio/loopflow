## Try it!

Inspect the current repository without mutating Linear:

```bash
cargo run -q -p loopflow --bin lf -- pm sync --plan
cargo test -p loopflow pm
```

The plan reports each legacy `pm.linear_project` wave that needs migration. In a
disposable Linear workspace, exercise the complete path:

```bash
cargo run -q -p loopflow --bin lf -- pm init --wave <wave>
cargo run -q -p loopflow --bin lf -- pm show --wave <wave>
cargo run -q -p loopflow --bin lf -- pm show --wave <wave> --json
```

`pm show` prints one aligned task record per line; `--json` emits the native
Initiative/Project task snapshot. Gate validation passed all six repository
suites plus `cargo clippy --all-targets -- -D warnings`.

## Intent

Make Linear's native hierarchy match Loopflow's planning model — Initiative →
wave, Project → measured bet, Issue → task — while removing daily credential
expiry as an operational interruption. Local project Markdown becomes a
generated offline cache instead of a competing source of truth.

## Assumptions

- Linear Project names derive unique, stable CLI slugs.
- Legacy tasks carry exactly one recognized `project:<slug>` label before they
  migrate; ambiguous tasks remain untouched for explicit repair.
- Existing OAuth rows either have legacy client credentials available or can be
  reconnected once to persist their PKCE client ID.
- Project definition/KR edits happen in Linear; `lf pm sync` refreshes the local
  cache afterward.

## Key decisions

- Persist the non-secret OAuth client ID beside the encrypted token record and
  refresh 20 minutes before expiry, preserving refresh-token rotation.
- Keep `pm.linear_project` until every legacy task migrates, and use a transient
  seed marker so interrupted Initiative setup resumes without duplication.
- Fail on duplicate derived project slugs instead of choosing a destination.
- Fetch per-project issue lists concurrently on the interactive show path and
  render stable full-ID table rows.
- Pin Linear's `String` GraphQL ID declarations in tests; HTTP mocks alone do
  not validate operation types.

## Not included

- Automatic migration on install/startup.
- Direct CLI editing of Linear Project definitions or KRs.
- Automated KR evidence and non-Linear hierarchy providers.
