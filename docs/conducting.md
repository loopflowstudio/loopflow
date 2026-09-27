# Conducting

Read this when work is running and you want to know where it stands.

Working with one AI agent is a conversation. Working with several means
keeping track: what is moving, what is stuck, what is waiting on you, and
what it cost. Conducting is how you see all of that and step in without
stopping everything else.

## In the Mac app

The app shows your repositories, their [Waves](glossary.md#loopflows-words),
and a roadmap of their Tasks on this computer. A Wave keeps working toward
an objective; a Task is one piece of work with a finish line. The app reads
the same records as the commands below.

- **Wave chat** is the ongoing conversation with a Wave. Send a message or
  interrupt it.
- **Roadmap** groups Tasks by what needs attention. Each Task also has a
  condition: clear, waiting, blocked, or unknown.
- **Sessions** are open conversations, including reviews waiting for you.
  Each [Session](glossary.md#loopflows-words) has a terminal inside the app
  where you talk to the AI directly.
- **Task workspace** shows the files a Task changed and the changes in each.
- **Telemetry** shows recorded usage, project growth, and the health of
  Loopflow's records. [Telemetry](glossary.md#borrowed-from-software-engineering)
  means measurements collected while software runs.

## From the command line

Type these commands in a shell, rather than in the AI conversation. Replace
`<wave>` with a Wave name, `INF-123` with a Task's Linear issue identifier, and
`<home-id>` with the identifier of another computer running Loopflow. That
computer is called a [Home](glossary.md#loopflows-words); `lf home id` prints
the identifier on that computer.

```bash
lf roadmap                # Tasks in this repository, grouped by what they need
lf roadmap --all          # every repository on this computer
lf top                    # what is running right now
lf runs --wave infra      # records of AI launches for the infra Wave
lf ssh <home-id> roadmap   # ask another computer the same question
```

Each command reads the computer it runs on. Use
[SSH](glossary.md#borrowed-from-software-engineering), a connection for running
commands on another machine, to ask a different Home. Reads do not combine
records from every machine automatically.

## See everything

```bash
lf ls                  # every Wave, including stopped ones
lf status <wave>       # current chapter, Tasks, Runs, and conditions
lf roadmap             # every open Task across this repository's Waves
lf roadmap --all       # every repository on this machine
lf activity            # recorded changes, newest first
```

A Wave's current chapter is its plan for this period. `lf status` includes its
[metrics](glossary.md#loopflows-words): measurements with targets and dates.
`metric_portfolio` is the field containing these readings in
[JSON](glossary.md#borrowed-from-software-engineering), the structured output
requested with `--json`. The same field appears on each Wave in
`lf roadmap --json`. It reports Met or Missed, Unknown when evidence is missing
or too old, Unavailable when a source could not be read, and measurements still
being set up. The app displays these results without calculating them again.

The roadmap groups work into **Now** (running and advancing), **Waiting**,
**Available**, and **Later**. It combines the plan in Linear with current
execution evidence. A Task's condition describes what it needs; it does not
say whether a process is running. `lf session list` separately lists
conversations that have not been completed.

`lf activity` records Work creation, Runs, Task pull requests, and steering
messages. [Work](glossary.md#loopflows-words) is the saved planning record;
a [Run](glossary.md#loopflows-words) records one launch of the AI coding tool.
Filter activity with `--wave`, `--project`, or `--task`. Filtering happens
before `--limit` caps the results. Use `lf ps` for processes running now;
activity is history, so finished work stays there.

## Drill down

Replace `run_ab12` with a Run identifier from `lf runs`. A parent Run is the
launch that started another Run; its direct children are one level below it.

```bash
lf runs --wave infra            # recent Runs for one Wave on this Home
lf runs --project parser        # one chapter Project, filtered before the cap
lf runs --task INF-123 --json    # one Task's recorded launches
lf runs --parent run_ab12 --json # all direct children, without the recent cap
lf runs run_ab12 --final         # the saved final answer
lf runs run_ab12 --events        # recorded events in their original form
lf replay run_ab12               # repeat the recorded request as a new Run
```

Each Run has a [manifest](glossary.md#loopflows-words), the launch settings
saved before the AI starts. Conversation and usage records are append-only:
new entries are added without rewriting earlier ones. At most one terminal
receipt records how the Run ended; here *terminal* means finished, not a
command window. These bundles can be read without the planning database.

`--final` reads the saved final answer in Loopflow's common format. If that
receipt is absent, it labels the fallback and returns text from the coding
tool's last completed turn. It reports an error if neither exists.

[Replay](glossary.md#loopflows-words) starts a new Run using the saved prompt,
coding tool and model, turn limit, permission mode, capability flags, and
account ID. The source Run becomes its parent and is unchanged. Replay does
not open the planning database or recover a Task's Flow position.

## Health and usage

```bash
lf ps                   # processes running now, and which launched which
lf top                  # refresh elapsed time and process state
lf prune --dry-run      # preview stale process records that can be removed
lf usage --days 30      # recorded coding-tool usage for each Run
lf usage --task INF-123  # usage for one Task
lf usage --json         # structured Run records, newest first
lf tokens               # lines and tokens per directory; --days adds history
lf ci --since 7d        # how failed checks were detected, repaired, and landed
lf doctor               # check continuity and ownership of saved records
```

Start with `lf top` when work feels slow. A
[process](glossary.md#borrowed-from-software-engineering) is a program running
on the computer. `lf ps --json` gives another tool one structured snapshot.
Both commands show only processes the operating system still reports as live;
finished processes disappear.

Run `lf prune --dry-run` before cleanup. Plain `lf prune` removes stale
execution receipts and stops registered orphan OpenCode process groups. It
does not stop a coding-tool process just because its PID (process identifier)
looks familiar or appears in an old record.

`lf usage` totals counters reported by the coding tool in each Run bundle.
Missing usage remains missing: it neither blocks a launch nor becomes zero.
[Tokens](glossary.md#borrowed-from-software-engineering) measure text sent to
and returned by an AI. `lf ci` reads the local record of
[CI](glossary.md#borrowed-from-software-engineering), the automatic checks on
pull requests, rather than querying GitHub. It shows how much repair happened
without a person intervening.

## Steer

Use Wave chat for ongoing direction. Use a Task comment when the direction
belongs to one Task, so later steps can read it too.

```bash
lf --wave <wave> wave/operate "ship the parser fix first"
lf chat --follow                       # read and follow the Wave conversation
lf task steer INF-123 "smaller PR"     # save direction as a Linear comment
lf task interrupt INF-123              # end this turn so the next rereads direction
lf session list --json                 # conversations awaiting completion
lf session open <session-id> --json    # prepare one exact conversation for reopening
lf ask list --user --json              # requested conversations needing attention
lf ask open ask_...                     # open one Ask; use its listed identifier
```

Task steering posts a Linear comment. A comment written directly in Linear
also reaches the worker advancing that Task. Independent Runs receive no live
injection. An idle Task keeps comments without starting execution. Interrupt
ends the active turn so advancement rereads direction. A receipt that the
message was published or delivered does not prove the AI followed it. See
[The Agent API](agent-api.md#steer).

An [Ask](glossary.md#loopflows-words) opens a conversation in the caller's
checkout and waits until a person completes it. Use
`lf --as <work> : "<prompt>"` for another agent's perspective instead;
`<work>` identifies existing Work, for example `wave:infra`.

A Flow review preserves the exact step and Run between unattended steps.
Opening it resumes that coding tool's conversation; moving it from another
client stops that exact client first. The agent saves feedback and marks the
Session ready. **Ready does not complete the review.** Choose **Complete** in
the app, or run `lf session complete <session-id>`, to return the feedback to
the next Flow step. A later deciding step chooses whether to continue or repeat
work. Closing the terminal or exiting the coding tool does neither. Completing
a review does not approve a merge.

## Inspect and resume

```bash
lf top                      # live Loopflow processes
lf status shipper           # the Wave's work and what it needs
lf session open <id>        # start or resume a listed conversation
```

Open a Session in the app or CLI to return to the coding tool's conversation.
Task workspace shells run directly in the app's terminal. Use Task steering
for saved direction, an ordinary Run for another agent's perspective, and an
Ask when a new conversation with a person is needed.

## Next

[The Agent API →](agent-api.md) · [Waves →](waves.md) · [`lf` reference](lf.md)
