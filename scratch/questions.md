# Open choices (d92894)

Made headless at kickoff and concept review; each is reversible. Detail in
`plan.md`, "Choices made for review", and `concept-review.html`, "Questions".

- The patch goes in whole under 15,000 tokens, the rule `context: {diff:
  true}` and the Task workspace block already use. Kickoff had planned 100,000
  bytes. Or never inline, even for a two-line commit?
- `context: {diff: true}` moves onto the same `Change` type in this Task, and
  loses its 20-file and 800-line limits. Leave it for its own Task instead?
- The importance-ordered fallback has no launch that would use it: every
  harness lf starts can run `git diff`. Planned: not built. Build it anyway?
- Notes added under `scratch/` appear in a branch change's file list, marked
  as cleared at land. Leave them out instead?
- The commit writer always skips permission prompts; the PR writer follows
  `yolo`. Both now have to run a command on large changes. Left as is. Give
  the PR writer the same floor?
- AGENTS.md says `lf pr submit` runs `lf scratch clear`; in code only
  `ops/land.rs:171` and `lf scratch clear` itself do. Which is meant?
- `--diff patch` moves the section into the first turn, after the request
  (plan, choice 5). qa, 2026-11-18: as a listed `changes` file a haiku agent
  never opened it in three runs, and on a branch with only uncommitted work it
  answered "adds nothing beyond `main`". Keep it a listed file instead?
- `docs/config.md` "Raw Diff" said "Point to Git inspection commands rather
  than inlining a frozen patch". On `main` the gathered diff was counted and
  never delivered. Did that sentence mean never put patch text in a Skill's
  prompt, or only not to rely on it staying current? If the first, the section
  should carry the list and the command without the patch for Skills.
- With `diff_files` and no `diff`, the `changes` file still says `git diff
  main...HEAD`, whatever the default branch is. Left alone.
- An uncommitted change now lists untracked files, marked "no patch: read the
  file". Their contents are not inlined. Inline small ones?
- The change block counts toward the 122,880-byte terminal argument cap
  (`agent::validate_terminal_turn`), which only Pi and OpenCode terminal
  launches check. qa, 2026-11-18: on a 3,001-file branch the list alone was
  237,456 bytes; headless Claude answered and interactive Claude launched
  (its first turn goes over stdin). The list has no ceiling anywhere: about
  80,000 tokens at that size, in a `--diff patch` Skill and in the PR
  writer's prompt. Bound it (say, collapse a directory of several hundred
  files to one line with a count)?
- In the `<lf:change>` block every `$` in the patch reads `&#36;`, under the
  one-line notation key (`prompt::render_reference`), as in the clipboard and
  explicit docs. The commit and PR writers get the patch unescaped. A Skill
  reviewing shell or Swift string interpolation reads the encoded form; the
  read command still returns the real bytes. Left as the shared rule.
- `ops::run::tests::deleted_task_retains_context_and_run_attribution` is
  flaky, on code this branch does not touch (`ops/run.rs` is unchanged from
  `main`). 2026-11-18: two failures in six runs of `cargo test -p
  loopflow-execution --lib -- prompt::change:: attribution`, passing alone.
  The panic, at `ops/run.rs:1080`: `resumed.unwrap_err()` held "Native launch
  has no admitted invocation" (`harness/native_session.rs:356`,
  `journal::current_lf_process_id()` was `None`) where the test expects "was
  deleted". Not diagnosed here; it wants its own Task.
- For a renamed file the read command gets a second spelling with both paths
  (plan, "What remains"). With one path Git returns the whole file as added.
  Or list and read with `--no-renames`, so a move shows as a delete and an add?
- `--diff files` loads every changed file's contents and delivers only their
  paths, in a listed file; the `<lf:change>` list already holds them. Remove
  it in its own Task, leaving `--diff` on or off? Not filed.
