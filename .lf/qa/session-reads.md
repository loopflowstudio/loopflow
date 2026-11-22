Read when the change touches what a Session read reports: state, availability or the quiet deadline.

# Session state and availability

Set up a [private Machine](served-launches.md).

`session_activity` rows come from hooks. Before starting the server, put a
`claude` stand-in first on the private `PATH`. It reads `--settings`, consumes
the opening input, runs the `SessionStart` (source
`startup`) hook, then polls marker files: `prompt` runs the `UserPromptSubmit`
hook with Claude's callback JSON on stdin (`session_id`, `transcript_path`,
`cwd`, `hook_event_name`, `prompt`), `quit` exits. One released prompt writes a
reading with `yielded=0`; 120 s later the Session reads `available` with no
write. A `Stop` hook sets `yielded=1`, which makes an interactive Session
available at once and hides the deadline. Create a disposable Task with
`lf task create '<words>'`, then use `lf task checkout <id>` to start its work
session.

Watch it two ways, since they are different paths. `lf session list --all
--available --subscribe` reads the store inline (`lf server status` shows no
client) and prints a line when the list changes; keep its stdin open
(`sleep 150 | lf …`). The served path is a `session.list` subscribe on the
socket: `{"type":"subscribe","id":"b","operation":{"kind":"session.list",
"directory":…,"headless":true,"after":null,"limit":null,"task":null},
"select":["id","state","availability"],"resume":null,"sender":null}`; without
`sender` the server answers "no operation `session.list`". A wake with no
write shows as a new `reading` whose `read_at.sessions` is unchanged. After a
socket read times out, Python's `makefile` reader is dead: `select` on the
socket and split lines yourself.

Ending the stand-in gives `state: closed` in the list and `liveness: dead` in
`lf session card <id> --json`. `lf task checkout` does not start an ended
conversation again, and `lf task complete` leaves its Session open
(`completed_at` NULL): a finished Session needs `lf task retire` fifteen
minutes later.

The installed `lf` reads the same private store when the branch adds no
migration (`lf --version` names its commit), so run each read with both and
compare. The `condition.observed_at` and `open_argv` paths always differ.
Clean up the private server, holders and providers as described in
[resume](resume.md).
