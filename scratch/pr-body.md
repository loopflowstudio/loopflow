## Try It!

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test -p loopflow
```

With Asana auth configured, run:

```bash
lf op pm init --wave systems
```

When `.lf/config.yaml` does not set `asana.default_team`, project bootstrap creates or reuses an Asana team named after the GitHub repo slug, for example `owner/repo`.

## Intent

Let Loopflow drive multiple repositories without mixing every wave roadmap into one hardcoded `Waves` Asana team. Each repo now gets its own bootstrap team by default while preserving the explicit team GID override for installations that already know where projects should live.

## Assumptions

`lf op pm init` runs in a GitHub-backed checkout where `gh repo view --json nameWithOwner --jq .nameWithOwner` can infer the repo slug. Repos that cannot infer a slug should set `asana.default_team` in `.lf/config.yaml`.

## Key Decisions

The repo slug moved into `engine::git::github_repo_slug` so release and PM share one helper.

The slug is passed as `AsanaClient::bootstrap_team_name`, not stored in `AsanaConfig`, because it is runtime-derived context rather than deserialized config.

Only the project bootstrap path computes the slug. `show`, `update`, and `status` still build a normal Asana client and avoid an extra GitHub CLI call.

## Not Included

No change to existing linked waves. Once `pm.asana_project` is set, read and update operations continue using that project GID directly.

## Validation

`cargo fmt`

`cargo clippy --all-targets -- -D warnings`

`cargo test -p loopflow`
