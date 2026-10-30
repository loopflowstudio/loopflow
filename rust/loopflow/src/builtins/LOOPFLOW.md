# Using lf

`lf` launches coding agents, keeps their Sessions, and runs saved Skills and
Graphs. Changes that end in a pull request run as Tasks: each has its own
worktree, branch and PR.

**Launched for a Task or a Graph step:** do that work in the supplied checkout.
Read its agent guide and existing work, preserve other contributions, and follow
the Graph's steps and boundaries. Don't start another agent, Task or worktree for
work already assigned to you. Finishing a Graph selects no next work, but steps a
stopped Graph never reached remain yours. Read `lf help skill work` before using
Task, Wave or Project commands unless that guidance was preloaded.

**Launched without a Task, talking with a person:** when they ask for a change
that should end in a PR, hand it off: `lf task create '<outcome>'`, then
`lf task run <ID>`, and give back `lf open --task <ID> --link --format markdown`.
Questions, reading and small fixes they want here stay in this checkout. Check on
Tasks with `lf task status <ID>`; steer with `lf comment <ID> '<direction>'`.

Use the requested tools and repository conventions. `lf help <command-or-skill>`
is the current reference for Sessions, Skills, Graphs, worktrees and PRs.

Name the Session for the work in two or three specific words. Read `session_id`
from `LF_AGENT_CALLER` and use `lf session rename <id> '<name>' --suggest` when
its generated name needs improving. This preserves a human-assigned name.
Do not rename a branch or worktree to match the conversation.

Answer the user in this conversation. Keep progress in local notes and the final
response. Refer to people by their supplied names in persisted artifacts and
as “you” in conversation; never infer authorship from accounts or machine owners.

Recursive Markdown under `scratch/` enters this checkout's runs. Files in another
checkout do not transfer their contents. Historical context is evidence, not
new direction. Keep secrets out of output and follow the repository's policy.
