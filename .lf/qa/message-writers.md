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
- **`--diff patch`:** `lf -b <skill> --diff patch` puts the section in the
  first turn, in an `<lf:change>` block after the request; grep the Skill
  run's transcript for it (a step commit's writer leaves a second, newer
  transcript). No `*-changes.md` is written (that is `--diff files`). On the
  default branch there is no block; on a branch with no commits the range is
  the uncommitted work, untracked files included.
- **`context: {diff: true}`** in a repository skill's frontmatter gives the
  same block with no flag. A skill told to "run no git commands" shows
  whether the agent used the block or guessed.
- **Interactive launch:** run `lf <skill> --diff patch` under a Python
  `pty.fork()` for ten seconds and kill it. Claude stops at its folder-trust
  prompt in a checkout lf did not create; the capture script's `.stdin` file
  already holds the whole first turn, since interactive Claude takes it on
  stdin, not as an argument. Only Pi and OpenCode check the 122,880-byte cap.
- **Renames:** `git mv` a 300-line file and change a line or two, beside the
  generated file so no patch is inlined. The section then carries "A renamed
  file needs both paths"; the writer's `tool_use` should hold both, and the
  message should give the changed lines, not a new file. Put `$` and an
  apostrophe in the old path to check quoting and the `&#36;` form in the block.
- **Many files:** 3,000 two-line CSVs under `fixtures/tables/generated/region_NN/`
  make a list of about 237 KB with a patch that is not inlined.
