# Open choices (LOO-986 kickoff, headless)

- Tool directories are **appended** to `PATH` in lf's `main`, not prepended as
  `supervised_path` and Swift do. Appending never shadows a person's own `PATH`
  order; prepending would let `/opt/homebrew/bin` win over `/usr/bin` under
  launchd. Reversible in one line.
- The fix lives in lf's `main` for every lf process, not only where a provider's
  `Command` is built (the brief's idea), so `gh`, `git` and the holder resolve too.
- `lf open` passes `--agent` only from the invocation's `-a` or
  `LF_AGENT_OVERRIDE`. An agent caller started with `LF_AGENT_OVERRIDE` would
  still move the conversation; new Sessions already clear it
  (`journal/request.rs` `PROCESS_CONTEXT_ENV`), so this should not arise.

## Concept review (headless)

- Receipt writers are first-answer-wins (`hard_link`, no overwrite) so the
  launcher's exact error beats the viewer's scraped tail. Overwrite-last would
  also be reversible, but would report the noisier text.
- Dropping the caller's `PATH` (as the brief asks) loses nvm/mise/asdf tools for
  a person's own `lf open` into a launchd-started Rex. Alternatives not taken:
  carry `PATH` only when the caller is a person (`agent_caller()` none), or run
  the pane command through the person's login shell. See
  `scratch/concept-review.html` question 1.

## Implement (headless)

- `remote_path()` now **appends** the tool directories (was: prepended, with
  `~/.cargo/bin` first). A remote with both a dev `lf` in `~/.cargo/bin` and a
  release one in `~/.local/bin` now gets the remote shell's own order, then
  `~/.local/bin`. Reversible in one line.
- `end_open` answers only failures. A viewer that exits 0 before attaching
  still gets the wrapper's "(status 0)" line rather than its tail.
- The bare-PATH fixture hides `claude` only because no tool directory holds
  one; on a Machine with `claude` in `/opt/homebrew/bin` it would find it.
  Not changed here.

## Realign (headless)

- How the bare-PATH provider fixture should hide `claude` on a Machine that
  has it in a tool directory is left open; the three options are in
  `scratch/plan.md` "Remaining".

## QA (headless)

- `lf open --host tmux` failed on tmux 3.7c before any pane opened ("can't
  specify pane here": `new-window -a -t` no longer takes a pane). Fixed here by
  asking tmux for the caller's window first; it is not part of the three
  outcomes but blocked checking them on tmux.
- `~/.opencode/bin` joined the tool directories: OpenCode's installer uses only
  that directory. Reversible in one line.
- A provider that starts and exits at once is reported `usable`: the receipt is
  acknowledged when the provider process is spawned (`NativeStarted`), not when
  it takes input. The tail rung of part 2 was not reachable in the real product.
  Whether `usable` should wait for more is a product question, not changed.
- A failed `lf open -a <harness>` still records the move: the next `lf open`
  without `-a` tries that harness again. Not changed.
- Pi installed under nvm is "not installed" in a Rex pane on this Machine. This
  is the recorded cost of dropping the caller's `PATH`; before this change a
  terminal caller's `PATH` reached the pane.
