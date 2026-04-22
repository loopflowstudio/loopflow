## Try it!

```bash
lf op auth asana
lf op auth linear
lf op auth configure asana   # exits: Asana requires OAuth. Run 'lf op auth asana' to connect.
lf op auth configure linear  # exits: Linear requires OAuth. Run 'lf op auth linear' to connect.
lf op auth configure notion  # exits: Notion requires OAuth. Run 'lf op auth notion' to connect.

lf op pm init pm             # attaches/creates Asana's shared Working branch custom field
lf ops ingest --wave pm      # Linear/Asana claims verify the current branch as the worker lock
```

Validation run here:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
rg "ASANA_ACCESS_TOKEN|LINEAR_API_KEY" rust/
```

## Intent

PM auth now has one supported setup path: OAuth. The old Asana and Linear env-var/API-key setup paths were bootstrap residue and made failures point users at the wrong command. Claim coordination now uses each worker's branch name as the discriminator, so two workers sharing one PM account do not both win because they are the same assignee.

## Assumptions

- Worker branch names remain unique; `{user}.{name}.{timestamp}` uniqueness is load-bearing for same-account claim safety.
- Asana waves created before this change should run `lf op pm init <wave>` again to attach the `Working branch` custom field.
- PM OAuth client credentials still come from the lfd environment; this does not add a client-secret broker.

## Key decisions

- Kept API-key configure support for model/coding providers only: Claude, Codex, and OpenCode Zen.
- Left stale PM API-key rows in the credential store alone; removed entrypoints rather than adding migration code.
- Used one Asana workspace-level text custom field named `Working branch`, attached per project.
- Kept Asana working-branch comments as a UI activity trail; the custom field is the lock.
- Deleted the PAT-based `scripts/setup-asana.py` helper instead of preserving a second setup path.

## Not included

- Notion claim locking; it remains best-effort and arbitrated at PR time.
- Typed auth-step capability declarations for Swift/Rust UI.
- Default OAuth app credentials or secret injection automation.
