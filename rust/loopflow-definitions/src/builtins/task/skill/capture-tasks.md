---
description: Explore an idea and file useful Tasks in the Waves they belong to, across repositories.
requires: none
action_style: exploratory
---
Turn an idea into Tasks the right Waves can pick up later.

## Explore the idea

Start from the idea and the conversation behind it; don't redo settled
discovery. Explore the experience wanted, the alternatives, the constraints
and what success looks like, as deeply as the idea needs. Keep the ambition: a
Task doesn't need a technical design to be filed.

Favor larger, cohesive Tasks with a clear promise of what changes for whom.
Split where outcomes deliver independently or parallelism truly helps; one Task
can still publish several PRs. Speculative implementation steps stay inside
the Task or its design.

## Find where each Task belongs

The repository and Wave you started in are clues, not limits. Each Task belongs
to one Wave. In each destination repository, run `lf wave list --json` and
`lf wave status <wave> --json` to read objectives, plans and overlap; run every
command from that repository. Refine an existing Task for the same work rather
than filing a duplicate.

Plans don't show which open PRs already rewrite the code. Before filing work
against specific files, list the PRs that change them
(`gh pr list --json number,title,headRefName,files`) and name each overlap in
the brief. A choice another Task leaves open stays open.

## File it

File once intent is clear. Show the brief and file it; no extra round of
confirmation.

Each Task stands alone: the problem, who benefits, the outcome wanted, how to
tell it's done, real constraints and the open decisions that matter. Keep what
people asked for, quoted, apart from possible mechanisms. The essential intent
goes in the Task itself; a transcript or a path in another checkout doesn't
travel. Dependent Tasks each get their own design excerpt, never a shared plan
with a "current slice" marker.

```bash
lf task create --wave <wave> --title "<outcome>"    # brief on stdin or --notes; no --run
lf task update <issue>                               # refine a filed Task
lf --task <issue> open --link --format markdown      # the link to hand back
```

Return each Task as that one link, with its repository and Wave. A printed
link opens nothing; run `lf open '<url>'` only when asked.

Every `lf task create` makes a new Task, even with identical input. After a
write that may have failed, check `lf wave status <wave> --json` before
anything else, and update the Task if it exists. If you can't tell, report
that rather than creating again. Keep the filings that succeeded when another
fails, and leave an unfiled brief in the conversation with the reason.

Filing starts nothing. Stay in this conversation's repository and Wave for the
next idea.
