Read when the change touches what the commit or PR message writers, or `--diff patch`, are shown of a change.

# Message writers and the change section

A throwaway repository, a private `LF_HOME` and the account's own `claude` or
`codex` login are enough; `-a claude:haiku` answers in about ten seconds. Run
the branch `lf` through a wrapper that unsets `LF_*` and
`CLAUDE_CODE_CHILD_SESSION`, sets `LF_BIN` and closes stdin.

To make the change large, stage a generated file that sorts first
(6,000 lines, about 350 KB) beside a short source file that sorts last and
holds the real change, with its reason in a code comment. The message should
name the short file's change. Under 15,000 tokens the patch is inlined
instead, so test both sizes.

- **What the writer was sent:** put a `claude` script ahead of the real one on
  `PATH` that writes `"$@"` to a file and runs `tee <capture> | <real claude> "$@"`.
  The prompt is one stream-json line on stdin. lf also calls
  `claude --version`, so expect an empty capture beside it.
- **What the writer read:** its transcript is
  `~/.claude/projects/-private-tmp-<path with / as ->/<id>.jsonl`; the
  `tool_use` entries hold the commands it ran.
- **`lf commit`** stages everything, so the fixture needs no `git add`.
  With nothing changed it prints "Nothing to commit" and launches nobody.
- **Step commit:** a repository skill that edits files and is told to run no
  git commands, run with `lf -b <skill> -a claude:haiku`; the commit is titled
  `lf <skill>: …`. `describe` is taken by an lf command; pick another name.
- **PR description:** the stand-in GitHub in `stacked-tasks.md`, with the real
  `claude` left on `PATH`. `lf pr publish -a claude:haiku` writes the title
  and body into `$GH_QA_STATE`. The PR writer runs without
  `--dangerously-skip-permissions` unless `yolo` is set and can still run
  `git diff`.
- **`--diff patch`:** `lf -b <skill> --diff patch` writes
  `.lf/prompts/*-changes.md`. The agent is not shown it, only its path in the
  context listing, so check the transcript before concluding it was read.
  On the default branch no file is written; on a branch with no commits the
  range is the uncommitted work.
