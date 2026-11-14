# Using lf

`lf` launches coding agents, keeps their Sessions, and runs Skills and Graphs.
Work that ends in a pull request runs as a Task, with its own worktree, branch
and PR.

**In a Task or a Graph step:** do that work in this checkout. Read the agent
guide and what is already there, keep other people's changes, and follow the
Graph's steps. Don't start another agent, Task or worktree for work that is
already yours. Read `lf help skill work` before using Task, Wave or Project
commands, unless it is already loaded.

**In a conversation with no Task:** questions, reading and small fixes happen
here. A change that should end in a PR becomes a Task: `lf task create
'<outcome>'`, `lf task run <ID>`, then give back
`lf open --task <ID> --link --format markdown`. Check on it with
`lf task status <ID>`; steer it with `lf steer <ID> '<direction>'`.

`lf help <command-or-skill>` is the current reference.

Name the Session for its work in two or three words: take its ID from the lf
context block and run `lf session rename <id> '<name>' --suggest`. A name a
person gave is kept. Don't rename branches or worktrees.

Answer in this conversation. Never `lf resume` your own Session. Docs, PRs, Task
briefs and memory describe the work as it is, without saying who asked for it.

Write down what can't be worked out again later: a person's exact words, what a
run showed, outside facts. Code, history and a fresh run can recover the rest.
Quote what people said; don't turn a reply into a verdict like "accepted". Aim
for quality per word, not more words.

Markdown under `scratch/` is loaded into this checkout's runs. Another
checkout's files are not. Old notes are background, not new instructions.
Keep secrets out of output.
