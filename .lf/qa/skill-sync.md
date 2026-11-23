Read when the change touches `lf self sync-skills`, the directories it writes or what a harness lists from them.

# Skill sync against the real harnesses

Files first, in a throwaway home: `HOME=/tmp/lf-qa-*/home lf self sync-skills --yes < /dev/null`
with inherited `LF_*` unset. `dirs::home_dir` follows `HOME`, and personal
skills are read from `$HOME/.lf/skills/<name>.md`, not `$LF_HOME`. For a
returning home, sync it with the installed `lf` (`whence -p lf`) first, then
with the branch, and `diff -r` each tree against a copy. The confirmation
prompt needs a PTY (`pty.fork()`, answer after the `?`).

Listing needs each harness's real login, so those checks use the real `HOME`;
a probe skill `~/.lf/skills/qa-probe-<word>.md` plus a sync adds one export to
each tree, and removing it plus a sync prunes them. None of these calls a model:

- Antigravity: `agy -p "/skills" < /dev/null | cut -f1` (name, tab,
  description; about 3 s). With another `HOME` it asks for a Google sign-in and
  times out after 60 s. In an open session `/skills reload` prints
  "Reloaded 117 skills: 1 added, 0 updated, 0 removed."; from a PTY a new
  directory first shows "Do you trust the contents of this project?" (Enter).
- Pi: `(echo '{"id":"1","type":"get_commands"}'; sleep 6) | pi --mode rpc --no-session`
  and read `data.commands` where `source` is `skill` (`sourceInfo.path` names
  the directory). A repository's `.agents/skills` appear only with `--approve`
  or a saved trust decision.
- Codex: app-server `skills/list`.
- OpenCode has no listing command; `opencode run --standalone --auto -m opencode/<a -free model> '<ask for a fact only the skill holds>'`
  prints `Skill "<name>"` when it loads one.

To see a harness use a skill, give a repository skill a passphrase and ask for
it: `agy -p '<question>'` answers on the free tier. Pi's stored Anthropic login
may fail to refresh and OpenCode credits run out; say so and move on.

A fixture repository needs only `git init`, `.lf/skills/<name>.md` and
`.lf/graphs/<name>.yaml`; `lf self sync-skills --repo` writes `.claude/skills`
and `.agents/skills` there and nothing in the home.
