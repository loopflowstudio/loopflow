# Open questions

- The branch already contains a partial implementation of item 01 centered on runtime journal emission/observation, but the larger shared `FlowEngine` / `lfd` real-CLI executor refactor from `scratch/lfd-real-cli-executor.md` is not yet present. I treated `LF_RUN_ID` journal correlation as the highest-leverage missing piece in the current implementation rather than attempting the full daemon executor rewrite in one pass.
