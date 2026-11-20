# Glossary

Every term the docs use, in one plain sentence.

## Loopflow's words

| Term | What it means |
|---|---|
| **Skill** | One step: Markdown instructions for an agent in `.lf/skills/` (or a builtin). |
| **Graph** | A YAML workflow of Skills, commands, loops, and branches in `.lf/graphs/` (or a builtin). |
| **Workflow** | The Graph a Task took up and the step it currently stands at. |
| **Interactive step** | A Graph step written `interactive: <skill>`, where work pauses for human input (`-b` runs it headless). |
| **Loopflow** | A graph that can go back and try again. Exactly: a graph with at least one backward edge. |
| **Deciding step** | The final Skill of a `loop:` block, returning `advance`, `iterate`, or `blocked`. |
| **Task** | One unit of work with its own worktree, branch, persistent conversation, workflow, and pull request. |
| **Wave** | An ongoing area of work with a `GOAL.md`, curated `MEMORY.md`, KRs, and shared defaults for its Tasks. |
| **Plan** | Shared Waves, Projects, and Tasks (`lf wave plan`, `lf plan list`). |
| **KR** | Key result: an observable outcome stated for a Wave's Project. |
| **Memory** | `MEMORY.md`, where a Wave keeps decisions and context across Tasks. |
| **Session** (**LfSession**) | A continuable agent conversation (interactive or headless) with identity, steers, and native provider history. |
| **Work session** | A Task, Wave, or repository conversation running in a held terminal; survives window close and `lf install`, and wakes on steers. |
| **Worker** | A headless step of a Task Graph (or Discord Wave reply) whose turns are reported to the server. |
| **Chat** | A direct terminal conversation (`lf`, `lf <skill>`) that runs straight in your terminal and ends with it. |
| **Batch** | A direct headless run (`lf -b <skill>`) that prints its answer and exits. |
| **Available** | A conversation ready for your next turn (its last turn ended or it asked for input). |
| **Machine** | One OS user's Loopflow authority and data directory, used as a local or SSH execution destination. |
| **Server** | The local client-access process on a Machine that connects Desktop, starts work sessions in a holder, and records turns. |
| **Holder** | The process beside the server that keeps held work-session terminals and Desktop shells alive across `lf install`. |
| **Data directory** | Local Loopflow state (`~/.lf` by default; `LF_HOME` overrides it). |
| **Steer** | A message sent to running or idle work to redirect it at its next turn or step. |
| **Harness** / **provider** | The coding agent Loopflow drives: Claude Code, Codex, OpenCode, Pi, or Antigravity. |
| **Scratch** | The `scratch/` directory for temporary working notes; `lf scratch clear` removes added files on submit/land. |

Work session, worker, chat, and batch are launch kinds; a Session can change
kind between launches (for example, binding a chat to a Task reopens it as a
work session). Contributors looking for internal runtime contracts can check
[cutover status](dev/reference-cutover.md#cutover-status).

## Borrowed from software engineering

| Term | What it means |
|---|---|
| **Worktree** | A separate on-disk checkout of a Git repo on its own branch, so parallel Tasks never overwrite each other. |
| **Sync** | Merging updates from `main` (or a stack parent) into a feature branch while keeping its commit history. |
| **Tracker** | Linear or Asana, when a repository shares its Tasks and Wave plans there with coworkers. |
| **Hub** | An always-on Machine your other Machines exchange the plan with in seconds, for every repository without a tracker. Without one the plan still travels through Git. Set with `lf machine hub`. |
| **Pull request** (**PR**) | A proposal to merge a branch into `main` (or a stack parent) on GitHub. |
| **Land** | Merging a PR into its base branch (`lf land --wait-and-fix` repairs CI until it merges). |
| **CI** | Automated tests and lint checks that run on every PR. |
| **Token** | The unit LLMs measure text and cost in (roughly three quarters of a word). |
