---
name: loopflow
description: Hand changes off as Loopflow Tasks that each end in a pull request; check on, steer, resume and land them; run the repository's Skills and Graphs with lf.
---

# Work through lf

`lf` launches coding agents and keeps their sessions. Hand work off through it,
and keep this conversation free to talk.

## Hand off changes as Tasks

When the person asks for a change that should end in a pull request, make it a
Task and run it, one Task per reviewable outcome:

```bash
lf task create <<'EOF'                         # prints the Task ID
<outcome in the person's words: the first line is the title>
EOF
lf task run <ID>                               # own worktree and branch; returns once running
lf open --task <ID> --link --format markdown   # the link to give back
```

The person's words are data, never shell: send them on stdin through a quoted
heredoc as above, or as one single-quoted argument with each `'` written `'\''`.

Give back each Task's link and say what it will deliver. Questions, reading
code and one-line fixes the person wants here happen in this checkout, without
a Task. When unsure whether something is one Task or several, propose the split
and ask.

## Keep track

```bash
lf task list                          # open Tasks
lf task status <ID>                   # its run, its PR, what it needs from the person
lf comment <ID> '<direction>'         # reaches the run before its next step
lf resume --task <ID>                 # the Task's conversation, for the person
lf land --wait-and-fix                # only when the person asks to land
```

Report what status says. A finished run is not a merged PR, and a merged PR is
not a completed Task; say which happened.

## Skills and Graphs

Run a Skill or Graph only when the person asks for it, by name or by what it
does. They are the repository's own prompts and steps, so read one with
`lf help <name>` and say what it will do before running it: `lf <skill>` here,
or `lf task run <ID> <graph>` on a Task. `lf list` shows what exists.

## Boundaries

- Use the agent, account and machine the person names (`-a`, `--account`,
  `--machine`); otherwise lf's defaults.
- One run per Task at a time: check `lf task status <ID>` before starting another.
- Never invent a Task identity or act on a Task nobody named.
- `lf help <command>` is the current reference; check it before using a command
  for the first time. Docs: https://loopflow.studio/docs/agent-api.md
