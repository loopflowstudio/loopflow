# Glossary

Every term the docs use, in one plain sentence. Loopflow's own words come
first, then the engineering words it borrows.

## Loopflow's words

| Term | What it means |
|---|---|
| **Skill** | One step: instructions for the AI, written as a text file in `.lf/skills/`. |
| **Graph** | Steps in order, written as a file in `.lf/graphs/`. |
| **Workflow** | A Task's workflow: the graph it took up, and the step it stands at. Any graph in `.lf/graphs/` or the builtins can be one; there are no separate workflow files. |
| **Interactive step** | A Graph step written `interactive: <skill>`, where a person takes part: `lf graph` runs it with you, `lf task run` stops before it for the Task conversation, `-b` runs it headless. |
| **Loopflow** | A graph that can go back and try again. Exactly: a graph with at least one backward edge. |
| **Deciding step** | The step that chooses between moving on and going back. |
| **Task** | One piece of work with a finish line. |
| **Wave** | A goal Loopflow keeps working on. It has a written objective, a memory, and a schedule. |
| **Chapter** | The shared name of every Wave's In Progress Project; it has no separate stored object. |
| **Project** | One Wave's plan: status, Tasks, KRs and targets. |
| **KR** | Key result. A result you can check, stated before the work starts. |
| **Memory** | A text file, `MEMORY.md`, where a Wave writes down what it learned. |
| **LfSession**, **Session** | One continuable agent conversation, interactive or headless, with identity, feedback and native history. |
| **Run** | What the product calls a headless Session; an interactive one is a Session. |
| **Available** | A conversation you can work with now: it asked a question, handed its turn back, or went quiet with no tool call outstanding. Availability needs no pending question or review, and owes nothing; a Task's checks or dependencies neither add nor remove it. |
| **LfProcess**, **Process** | One actual lf process: durable `id`, optional Unix `pid`, causal parent and observed command outcome. |
| **AgentProcess** | The provider's OS process Loopflow started for a Session; it keeps running when another lf process attaches. |
| **Machine** | One OS user’s Loopflow authority in one data directory; two users on one host have separate identities. |
| **Data directory** | Local Loopflow state in `~/.lf` by default; explicit `LF_HOME` selects a disposable experiment. |
| **Steer** | A message you send to work that is already running, to change its direction. |
| **Harness**, **provider** | The AI coding tool Loopflow drives: Claude Code, Codex, OpenCode, or Pi. |
| **Scratch** | The `scratch/` folder, for working notes. `lf scratch clear` removes added files; existing files and edits remain. |

Historical capture paths and immutable receipts retain their encoding; see
[cutover status](architecture/reference-cutover.md#cutover-status). They are not a fourth
execution owner in the accepted model.

## Borrowed from software engineering

| Term | What it means |
|---|---|
| **Terminal** | A window where you type commands instead of clicking. |
| **Command line**, **CLI** | A program you use by typing in a terminal. Loopflow's is `lf`. |
| **Repository**, **repo** | A project folder whose history is tracked by Git. |
| **Git** | The tool that records every change to a project, so you can see or undo any of them. |
| **Commit** | One saved change, with a note saying what changed and why. |
| **Branch** | A separate line of changes, kept apart from the main version until it is ready. |
| **Main** | The branch that holds the finished, agreed version of the project. |
| **Worktree** | A second copy of your project on disk, on its own branch. Loopflow gives each task one, so tasks never overwrite each other. |
| **Pull request**, **PR** | A proposal to add a branch's changes to main, where they can be reviewed first. |
| **Merge**, **land** | Adding a branch's changes to main. |
| **Sync** | Merging updates from main or a stack parent into your branch while keeping its commit history. |
| **CI** | Continuous integration: automatic checks that run on every pull request. |
| **Test** | A small program that checks another program does what it should. |
| **Lint** | An automatic check for mistakes and style, without running the code. |
| **GitHub** | A website that stores repositories and hosts pull requests. |
| **Planning provider** | Where your Tasks and Projects are shared: a Git ref by default, or Linear, Asana or Notion. |
| **Linear** | A planning tool. Loopflow can keep Tasks and Projects there instead of in Git. |
| **Token** | The unit AI tools count text in, and charge by. Roughly three quarters of a word. |
| **YAML**, **Markdown** | Two plain text formats. Graphs are written in YAML; skills and docs in Markdown. |
