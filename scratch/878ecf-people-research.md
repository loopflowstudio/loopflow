# 878ecf research: people, teams, Machines (2026-10-21)

Requested by Jack Heart in design review. Inputs to the next design pass.

## Loopflow today (HEAD edcc997a0d)

- A person is a display name: `config::participant_name()` (LF_USER_NAME →
  lf config user.name → git user.name), compared as a string
  (`task_ownership.rs:199-205`). No users table; no lookup by email anywhere.
- `planning_user(user_key)` is a random UUID naming
  `refs/loopflow/planning/users/{key}`; not a person identity.
- Machines (`durable.rs:44-47`, `machines` table) are local-only, added by
  `lf machine add`; never exchanged. No owner/public/default fields.
  Docs: "Nothing routes to another person's Machine" (`machines.md:143`).
- Git planning ref exchanges waves, projects, tasks, task_comments only.
  Ownership travels as `loopflow-ownership` marker comments.
- Linear assignee = this Machine's Linear viewer (`assert_assignee`).
- Asana/Notion providers and `PlanningClient` exist only on unmerged
  `jack/loo-520-…` (16 commits ahead). No list-users / by-email ops there.

## Provider user/team models

| | Linear | Asana | Notion |
|---|---|---|---|
| id | UUID | gid | UUID |
| email | always | opt_fields | only with email capability |
| by email | `users(filter:{email})` | `GET /users/{email}` | none: list + match |
| "me" | viewer | `/users/me` | bot (owner user via bot.owner) |
| teams | Team + TeamMembership | Team (orgs only) + membership | none in public API |
| assign | single `assigneeId`, notifies | single `assignee`, notifies | People property (array, users or groups), notifies |
| per-user metadata | none | user custom fields | none |

Intersection: user `{provider_id, name, email?, is_bot?, avatar_url?}`;
ops: list users, get by id, resolve by email (Notion: scan + capability),
assign one user (notifies). Teams are NOT in the intersection (Notion).
No provider can hold a Machine/default-Machine record portably.

## Consequences for the design

- Machine routing needs Loopflow-owned records regardless of provider, so
  Machine (and probably Person) records ride the Git ref even when Linear is
  the planning provider.
- Provider assignment is the free offline notification in all three.
- Email is the cross-provider join key; Notion may not give it.
