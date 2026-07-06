# Asana Team Bootstrap Review

## What Was Implemented

`lf op pm init` now creates or reuses an Asana team named after the GitHub repo slug, such as `owner/repo`, when no explicit Asana team GID is configured. The previous hardcoded `Waves` team name is gone.

The GitHub repo slug lookup moved from `ops::release` into `engine::git::github_repo_slug`, and release now calls the shared helper.

## Key Choices

The repo slug is runtime context, not config, so it lives on `AsanaClient` as `bootstrap_team_name: Option<String>` instead of being added to `AsanaConfig`.

Only `pm init` computes the slug. Read-only paths still build a plain Asana client and do not pay an extra `gh repo view` call.

An explicit `asana.default_team` GID still wins before any team listing or creation. If no explicit team is set and the slug cannot be inferred, project bootstrap returns an actionable error instead of falling back to a shared team.

## How It Fits Together

`pm_init_async` resolves the wave, skips work if `pm.asana_project` is already present, then derives a bootstrap team name only when needed. `AsanaClient::create_project` resolves workspace, resolves the bootstrap team by explicit GID or repo slug name, and creates the project in that team.

## Risks And Bottlenecks

Bootstrap still depends on the GitHub CLI being able to infer the current repository. That failure now produces a clear `PmError`, but users in non-GitHub remotes need to configure `asana.default_team`.

The repo slug team match is case-insensitive, matching the previous team reuse behavior.

## Not Included

This does not change `show`, `update`, or `status`; they still operate only from an existing `pm.asana_project` GID.

This does not add runtime-derived fields to `AsanaConfig`.
