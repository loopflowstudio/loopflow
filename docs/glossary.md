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
| **Plan** | The shared Waves, Projects and Tasks. `lf wave plan` shows one Wave’s part, and `lf plan list` shows the repository’s plan. |
| **KR** | Key result. A result you can check, stated before the work starts. |
| **Memory** | A text file, `MEMORY.md`, where a Wave writes down what it learned. |
| **LfSession**, **Session** | One continuable agent conversation, interactive or headless, with identity, feedback and native history. |
| **Managed** | A work session or worker that reports each turn to the Machine server. |
| **Bare** | A chat or batch with no turn reports to the Machine server. |
| **Work session** | The conversation of a Task, Wave or repository, such as `lf resume --task LOO-12`. The server keeps it running when its window closes and tells it about steers while it is idle. A managed, interactive launch in a held terminal. |
| **Worker** | A headless step of a Task's Graph, or a Discord Wave answer, that the server follows turn by turn. A managed, headless launch. |
| **Chat** | A conversation in your terminal, started with `lf <skill>` or plain `lf`, that ends with it. A bare, interactive launch. |
| **Batch** | One headless answer, started with `lf -b <skill>`, returned to its caller. A bare, headless launch. |
| **Available** | A conversation you can work with now: it asked a question, handed its turn back, or went quiet with no tool call outstanding. Availability needs no pending question or review, and owes nothing; a Task's checks or dependencies neither add nor remove it. |
| **LfProcess**, **Process** | One actual lf process: durable `id`, optional Unix `pid`, causal parent and observed command outcome. |
| **AgentProcess** | The provider's OS process Loopflow started for a Session: a chat uses its terminal, a batch follows its caller, a work session uses a holder, and a worker runs without a terminal. |
| **Machine** | One OS user’s Loopflow authority in one data directory, used as an execution destination; two users on one host have separate identities. |
| **Server** | The client access process on a Machine; it connects Desktop to Work and existing Sessions, starts work sessions in a holder, and records work session and worker turns. |
| **Holder** | The process beside the server that keeps work session terminals and agents, plus Desktop shells; one per build, and an install never restarts it. |
| **Installation** | The selected Loopflow build and its registration, updated with `lf install`, including the client server. |
| **Data directory** | Local Loopflow state in `~/.lf` by default; explicit `LF_HOME` selects a disposable experiment. |
| **Steer** | A message you send to work that is already running, to change its direction. |
| **Harness**, **provider** | The AI coding tool Loopflow drives: Claude Code, Codex, OpenCode, or Pi. |
| **Scratch** | The `scratch/` folder, for working notes. `lf scratch clear` removes added files; existing files and edits remain. |

Work session, worker, chat and batch are kinds of launch. A Session can change
kind between launches: a chat bound to a Task reopens as a work session.

Task membership is separate from managed or bare: the checkout, `--task` and
`LF_AS` choose which Task receives the launch's usage and supplies its steers.

Historical capture paths and immutable receipts retain their encoding; see
[cutover status](dev/reference-cutover.md#cutover-status). They are not a fourth
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
| **Tracker** | Where your Tasks and Wave planning are shared: a Git ref by default, or Linear or Asana. |
| **Linear** | A planning tool. Loopflow can keep Tasks and Wave planning there instead of in Git. |
| **Token** | The unit AI tools count text in, and charge by. Roughly three quarters of a word. |
| **YAML**, **Markdown** | Two plain text formats. Graphs are written in YAML; skills and docs in Markdown. |
