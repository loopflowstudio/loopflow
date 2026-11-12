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
   Later on November 11: #1879 merged and is in this branch through
   LOO-824 (#1887, based on LOO-823's #1877).

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

# LOO-824

Its choices and reversible assumptions are in the parent branch,
`62c5e537e:scratch/questions.md` and `62c5e537e:scratch/holder.md`.

## Assumptions, LOO-820, reversible

- `Holder` is `{ Caller, Owner, Server }`, not the plan's
  `{ Terminal, Server(PrimaryScope), Run }`: see `native-provider.md`
  ("Holder, built"). A headless `-b` resume keeps its old rule: only the
  command's `--task`/`--wave` makes it `Server`, not the row's WorkSource.

- A bare Pi resume whose minted id has no session file starts fresh on that
  `--session-id` (what Pi does alone, with its own warning) instead of
  refusing; a deleted file looks the same. The other option was a message
  saying no reply was ever recorded.
- A signal to lf while a bare provider leads the terminal is forwarded as
  SIGTERM: ctrlc does not say which signal arrived.
- A framed remote viewer (`lf ssh`, `LF_VIEWER_STREAM`) keeps an owner for a
  bare launch; direct output would be unframed on its stream.
- Superseded on sync: LOO-823 gives the provider its own foreground process
  group; lf uses the parent's ordinary signal handler.

## Steering bare Sessions (open, November 11)

Asked: with turn hooks gone, should `lf session steer` say a bare Session
gets the steer only at its next start/resume/compaction, or refuse bare
Sessions? Jack Heart: "i think maybe we should remove lf session steer and
only allow lf task steer?", then "that is basically what our arhcitecture
supports, right?" Gap: Wave and repository conversations are managed and are
steered today only by `lf session steer` with no ID ("the current Wave's or
repository's Sessions"); `lf task steer` cannot reach them.

## Sync onto LOO-824, November 11

Use LOO-823's foreground launch and signal handling from the parent;
keep LOO-820's resume beside a running bare agent and lock reacquisition
when delegation does not start a transport. Parent design and reduction
notes remain at `62c5e537e:scratch/holder.md` and
`62c5e537e:scratch/simplification-opportunities.md`; this checkout keeps
LOO-820's design and launch reduction plan.

## Check after syncing onto 62c5e537e, November 11

`cargo test -p loopflow --lib transport::` in a headless run: the four
`transport::viewer_tests::snapshot_preview_*` tests fail. The resumed stand-in
`claude` reports "/dev/tty: Device not configured" and exits 1. The sync
changed no code (the only diff from `4f9b7796ff` is scratch), so the failure
was already on the branch, or comes from running without a controlling
terminal. Gate reruns it from a terminal.
