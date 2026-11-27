Read when the change touches the system prompt a launch composes (mode text, context blocks) or the environment a provider inherits.

# Comparing what two builds hand a provider

Two disposable repositories and two `LF_HOME`s, one per build (the installed
`/Applications/Loopflow.app/Contents/MacOS/lf` and the branch
`target/debug/lf`), one shared isolated `HOME`, and the stand-in `claude` from
[skills](skills.md) writing argv, `env | sort` and the first stdin line to
files named by a `TAG` variable. Set `LF_BIN` to the build under test.

Each launch kind writes its system prompt under the checkout's `.lf/prompts/`
and names it in `--append-system-prompt-file`:

| Launch | Command | File | Mode text (last lines) |
| --- | --- | --- | --- |
| batch | `lf -b -a claude <skill> '<words>'` | `*-session.context.md` | "Run mode is headless." |
| chat | `script -q /dev/null lf -a claude <skill> '<words>'` | `*-<skill>.context.md` | "Someone is in this conversation." |
| Task conversation | `lf task create '<words>' --run --json` | `*-task-session.context.md` | "Someone is in this conversation." |
| Task Graph worker | the same Task's `- step: <skill>` | `*-session.context.md` | "Run mode is headless." |

`diff` the two builds' files after replacing the repository name with `sed`;
nothing else differs between runs. The recorded word is `"surface": "tui"` or
`"headless"` in `$LF_HOME/runs/<xx>/<capture>/manifest.json`.

The Task rows need the private server from [server](server.md) and the
`qa` Graph (`- step: <skill>` then `- interactive: demo`) pushed to a bare
origin. Afterwards stop `lf server run` and its child `lf holder run` by PID.

Environment: put a marker variable in the launching shell and grep the
stand-in's env file. A Task Graph worker loses `GH_TOKEN`; a batch, a chat and
the Task conversation keep it.

A glob that matches nothing leaves `sed` reading stdin inside `diff <(sed …)`:
check the file exists first.

# Wave config keys

`lf wave create <name>` then edit `.lf/wave/<name>/config.yaml` and commit.
`lf --wave <name> config settings` prints one `warning: …: skipping <key>:
unknown setting` per key the shared layer does not know and shows the values
that did apply; a launch (`lf -b --wave <name> <skill> '…'`) prints the same
warning twice on the installed build too. For the `GOAL.md` fallback, delete
`config.yaml` and put the keys in `GOAL.md` frontmatter;
`lf --wave <name> config set --shared <key> <value>` then writes a
`config.yaml` holding every frontmatter key the build keeps.
