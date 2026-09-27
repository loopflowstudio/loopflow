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
| **Chapter** | One planning period for a Wave. Each chapter has its own plan. |
| **Project** | The plan for a Wave's current chapter: the results to reach and the tasks to get there. |
| **KR** | Key result. A result you can check, stated before the work starts. |
| **Memory** | A text file, `MEMORY.md`, where a Wave writes down what it learned. |
| **Session** | One open conversation with the AI, shown as a terminal inside the app. |
| **Run** | A record of one time the AI coding tool was started: what it was told and what happened. |
| **Home** | One computer running Loopflow, with its own records. |
| **Steer** | A message you send to work that is already running, to change its direction. |
| **Harness**, **provider** | The AI coding tool Loopflow drives: Claude Code, Codex, or OpenCode. |
| **Scratch** | The `scratch/` folder, for working notes. It is cleared when the work lands. |

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
| **Rebase** | Replaying your changes on top of the newest main, so the two fit together. |
| **CI** | Continuous integration: automatic checks that run on every pull request. |
| **Test** | A small program that checks another program does what it should. |
| **Lint** | An automatic check for mistakes and style, without running the code. |
| **GitHub** | A website that stores repositories and hosts pull requests. |
| **Linear** | A planning tool. Loopflow keeps Tasks and Projects there. |
| **Token** | The unit AI tools count text in, and charge by. Roughly three quarters of a word. |
| **YAML**, **Markdown** | Two plain text formats. Flows are written in YAML; skills and docs in Markdown. |
