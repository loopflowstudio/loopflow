# LOO-824 exploration

The visions in [explore.html](explore.html) and `holder.md` are proposals;
none is selected. The page is organized by three goals: load, separation from
Rex, and the Desktop experience.

1. **1a** move existing viewer loops into the server; **1b** give held
   terminals their own model and server dispatch; **1c** 1b plus a screen
   journal on disk.
2. New process for an ended Task/Wave Session via plain `lf resume SESSION`:
   **2a** managed by Session association; **2b** managed only through a
   Task/Wave entry point or Desktop. Existing held processes stay held.
3. Emulator: **3a** in the server, holder keeps raw bytes (as said in
   LOO-819); **3b** in the holder beside the PTY, as Rex and Paseo do;
   **3c** run the mode probe through production Ghostty first, then pick.
4. Open views across a server restart: **4a** resume from a PTY byte
   position out of the holder's ring, no repaint; **4b** refresh from the
   rebuilt emulator, as the kickoff plan has it.

Said so far (Jack Heart): "1b seems right"; "ok 3a 4a"; "i think i want 2a".
The direction built from those is in `holder.md`, "Direction after explore".

## Kickoff assumptions, pending exploration

Reversible choices made in kickoff; the plan is `holder.md`.

- `lf resume SESSION` (no `--task`) of a Session bound to a Task or Wave is
  managed. The brief names Task/Wave commands and Desktop only.
- Desktop Task shells stay on their own `session serve` owner: they run
  without a server on headless Linux.
- A graceful server stop writes a Ghostty snapshot per held Session. Exact
  recovery requires a compatible revision and continuous retained bytes;
  ring + nudge also applies to incompatible upgrades. Terminal-mode recovery
  after a gap has not been verified.
- Ring 2 MiB per PTY; holder exits 60 s after its last child.

## Raised while writing the docs

Written into the docs as assumptions; none was said by anyone.

- `lf task run` with the server stopped: does the whole command fail with
  `Loopflow server is not running; run lf server status`, or does the Graph
  start and only the Task's background conversation go unstarted (as it
  already does past sixteen conversations)? `docs/troubleshooting.md` says the
  command stops and nothing starts; `docs/sessions.md` says a Graph already
  running keeps going.
- A CLI managed launch never starts the server itself (Desktop's first
  connection does ask the installation for readiness). On headless Linux with
  the user service disabled, `lf resume --task` therefore cannot open a
  conversation until the service is enabled. Should a managed launch ask for
  readiness the way Desktop does before failing?
- A killed holder ends every agent it holds, and the docs say so. Nothing
  restarts one. Is "views say the process is gone and offer resume" the whole
  experience, or should the server say which conversations were lost?

## Raised while implementing

- Store names are `held_terminals` / `held_terminal_id`: `processes.terminal_id`
  already holds Desktop pane placement. The docs say "terminal" throughout.
- Plain CLI views and the held child's own control requests reach the
  registry through `held.sock` directly; the `session.attach` ticket bridge
  to a held terminal exists for clients that only speak the server protocol
  (Desktop). `server/README.md` says every view goes through `session.attach`.
- `bridged` and the attach ticket carry both `owner_lf_process_id` and
  `terminal_id`, one of them null, instead of two shapes under one name.
- A lost `session.start` holder reply answers `outcome_unknown`; the caller
  repeats the same `request`.

## Sync onto main — November 10, 2026

Keep this Task’s `explore.html` and `questions.md`: main removed its scratch
files, while these contain the holder design choices and exact replies.
