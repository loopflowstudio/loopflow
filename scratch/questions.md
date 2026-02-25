# Open questions / follow-ups

- Stage 01 done-when specifies iPhone 16 + iPad Pro 11" (M4) simulators, but this environment only has iPhone 17 + iPad Pro 11" (M5). Validation was run on the available runtimes.
- RepoState and store classes are still located under `swift/Concerto/State/` in this draft. Stage 01 design calls for moving shared state into `swift/LoopflowCore/State/`; this extraction remains as follow-up refactor work.
