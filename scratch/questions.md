# LOO-820 choices

Retired by the November 11 direction ("i dont think this task really needs
anything rex specfiic"): history-only recovery after a Rex server death, and
cross-host control of a Rex-owned conversation.

Answered, Jack Heart, November 11:

1. Scope: "it is the actual cange." One PR that makes bare launches direct.
2. `lf resume` of a bare Session still running elsewhere: "a" — a second
   provider on the same thread; lf stops nothing.
3. Pi: "no idea, explore what the best answer is". Probe below.
4. Sequencing: "you cn wait for either of them to land. stack on whichever
   one you think will land 2nd". Read on November 11: LOO-821's PR #1879 is
   open against main and CLEAN; LOO-824 has no PR yet, is in QA, about 10k
   added lines. So LOO-824 lands second: wait for #1879 to merge and LOO-824
   to sync onto it, then stack on LOO-824.

## Pi probe, November 11 (pi 1.3.0)

Two `pi -p` runs in an isolated `--session-dir`, cwd in the scratchpad,
`PI_OFFLINE=1`, using this Machine's ambient Pi model setting (it answered
through `opencode`/`gemini-4-argon`; two short model calls).

- First run with `--session-id 8f0c5e2a-…-944455556666`: warned "No project
  session found with id '…'; creating a new session with that id." and wrote
  `<dir>/2026-11-11T11-22-55-913Z_<id>.jsonl`; its first line is
  `{"type":"session","id":"<id>","cwd":…}`.
- Second run with the same `--session-id`: no warning, same file, two user
  messages in it, and the model recalled the first answer ("ok").

So lf can mint the id before launch and pass `--session-id`, as Claude gets
`--session-id`; it needs no `session_start` report from a hook. The file name
is not predictable (timestamp prefix), but `*_<id>.jsonl` in Pi's session dir
for the cwd, or the header's `id`, locates it. Today lf's Pi thread is the
absolute file path (`harness/pi.rs::observe_native`); bare Pi would record
the id and resolve the file when LOO-822's transcript reader needs it.
Not checked: whether interactive Pi writes the file before the first message
(a bare run quit before any message would then have no transcript, which is
fine), and how `--session-id` behaves with `--session-dir` unset (default
`~/.pi/agent/sessions/--<cwd>--/`). Managed Pi (RPC, `--session <path>`) is
unaffected.

Recommendation: launch bare interactive Pi with `--session-id`.

# LOO-824 open choices

Exploration choices (1b, 2a, 3a, 4a) are answered; the quotes and the
direction built from them are in `holder.md`. Options as offered:
`a18d6a915:scratch/questions.md`.

## Assumptions, reversible

- Desktop Task shells stay on their own `session serve` owner until LOO-830.
- No snapshot on graceful stop: rebuild is ring replay plus a nudge;
  terminal-mode recovery after a gap is unverified (slice 0).
- Ring 2 MiB per PTY; holder exits 60 s after its last child.
- Store names are `held_terminals` / `held_terminal_id`: `processes.terminal_id`
  already holds Desktop pane placement. The docs say "terminal" throughout.
- Plain CLI views and the held child's control requests reach the registry
  through `held.sock` directly; the `session.attach` ticket bridge serves
  clients that speak only the server protocol (Desktop).
- `bridged` and the attach ticket carry both `owner_lf_process_id` and
  `terminal_id`, one of them null, instead of two shapes under one name.
- A lost `session.start` holder reply answers `outcome_unknown`; the caller
  repeats the same `request`.
- `session.attach` for a pending holder's terminal answers `unavailable` with
  `retry: true`: the holder is a dependency not answering, not the server at
  capacity. No client reads `retry` yet.
- A replaced view (another `held.sock` revision) does not carry `--moved` or
  `--session-json` into the installed `lf`.
- Nothing yet lists which conversations a killed holder took.
