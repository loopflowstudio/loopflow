# Assumptions — 2026-09-30

- Jack Heart's explicit disposable-Home direction means `LF_HOME=/absolute/path`
  remains the experiment selector. Creating a fresh schema is supported;
  maintaining or migrating an existing experiment is not.
- Explicit experiment commands stay in that experiment, including their children.
  Default Task execution never chooses an experiment or copies main data into one.
- Local builds remain under `local-bin/`; installing the main CLI remains a
  published-release operation. No local-build promotion replacement is needed.

- Operational retirement stops at live file handles. The cleanup receipt records
  five stores still in use, including a snapshot created by another running
  source build during cleanup. Their owners must settle before deletion; the
  published cutover prevents implicit recreation. This does not establish Task
  completion or installed acceptance.
