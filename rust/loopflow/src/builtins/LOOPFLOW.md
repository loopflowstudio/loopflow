# Using lf

`lf` launches coding agents, keeps their Sessions, and runs saved Skills and
Flows. Use as much of it as the request needs. A Flow is an authored sequence
of steps; it runs with or without a Task.

Work in the supplied checkout. Read its agent guide and existing work, preserve
other contributions, and carry out the request here. Do not launch another
agent or create a worktree merely to do the work already assigned to this one.
Follow a supplied Flow's steps and boundaries; finishing it selects no next work.

Use the requested tools and repository conventions. Read
`lf help <command-or-skill>` for instructions on Sessions, Skills, Flows,
worktrees and PRs.

When continuing an existing Task or when the request calls for Tasks, Waves,
or Projects, read `lf help skill work` unless that guidance was preloaded. It is also
available as `lf skill work`. Work association supplies context; the request
or selected Skill/Flow determines what to do. Stay within that chosen scope.

Name the Session for the work in two or three specific words. Read `session_id`
from `LF_AGENT_CALLER` and use `lf session rename <id> "<name>" --suggest` when
its generated name needs improving. This preserves a human-assigned name.
Do not rename a branch or worktree to match the conversation.

Answer the user in this conversation. Keep progress in local notes and the final
response. Refer to people by their supplied names in persisted artifacts and
as “you” in conversation; never infer authorship from accounts or machine owners.

Recursive Markdown under `scratch/` enters this checkout's runs. Files in another
checkout do not transfer their contents. Historical context is evidence, not
new direction. Keep secrets out of output and follow the repository's policy.
