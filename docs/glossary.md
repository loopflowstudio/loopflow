# Glossary

Every term the docs use, in one plain sentence. Loopflow's own words come
first, then the engineering words it borrows.

## Loopflow's words

| Term | What it means |
|---|---|
| **Skill** | One step: instructions for the AI, written as a text file in `.lf/skills/`. |
| **Flow** | Steps in order, written as a file in `.lf/flows/`. |
| **Loopflow** | A flow that can go back and try again. Exactly: a flow with at least one backward edge. |
| **Deciding step** | The step that chooses between moving on and going back. |
| **Human step** | A step that waits for a person. In a flow file it is marked `human: true`. |
| **Task** | One piece of work with a finish line. |
| **Wave** | A goal Loopflow keeps working on. It has a written objective, a memory, and a schedule. |
| **Chapter** | The shared name of every Wave's In Progress Linear Project; it has no separate stored object. |
| **Project** | One Wave's Linear plan: status, Tasks, KRs, targets and default Flow. |
| **KR** | Key result. A result you can check, stated before the work starts. |
| **Memory** | A text file, `MEMORY.md`, where a Wave writes down what it learned. |
| **AgentSession**, **Session** | One continuable agent conversation, interactive or headless, with identity, feedback and native history. |
| **Exec** | One actual lf process, its causal parent and its observed command outcome. |
| **FlowSession** | One captured resumable Flow, its cursor and exact boundary completion history. |
| **Home** | A stable execution destination on a machine, where work for a Wave can be assigned. |
| **Data directory** | Local Loopflow state selected by `LF_HOME`; a branch data copy preserves recorded Home identities without creating another execution destination. |
| **Steer** | A message you send to work that is already running, to change its direction. |
| **Harness**, **provider** | The AI coding tool Loopflow drives: Claude Code, Codex, or OpenCode. |
| **Scratch** | The `scratch/` folder, for working notes. It is cleared when the work lands. |

Historical `Run` names remain on transitional CLI/wire surfaces; see
[cutover status](architecture-reference.md#cutover-status). They are not a fourth
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
| **Linear** | A planning tool. Loopflow keeps Tasks and Projects there. |
| **Token** | The unit AI tools count text in, and charge by. Roughly three quarters of a word. |
| **YAML**, **Markdown** | Two plain text formats. Flows are written in YAML; skills and docs in Markdown. |
