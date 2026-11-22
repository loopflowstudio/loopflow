# LOO-912: interrupted pastes — merged; what remains is verify and complete

## State (read 2026-11-21)

Nothing is left to build. PR #1944 merged 2026-11-15T08:06Z as `19872301e3`,
every CI check green. The installed `lf 0.16.0+a4589ee2a` contains that commit
(`git merge-base --is-ancestor 19872301e3 a4589ee2a`). No release tag contains
it yet; v0.16.0 predates it.

This checkout (`jack-heart/stuck-sessions`, HEAD `9ea8c43745`) is the
pre-squash branch, 53 commits behind `origin/main`. Its diff is already on
main; a build step here would re-apply merged work.

The code moved after the merge. #1954 split it into `loopflow-execution`:

| Was (this branch) | Is (origin/main) |
| --- | --- |
| `rust/loopflow/src/transport/input.rs` (`Paste`, `PendingInput::finish_paste`) | `rust/loopflow-execution/src/transport/input.rs` |
| `rust/loopflow/src/server/holder.rs` (`close_input`, `input_paste`) | `rust/loopflow-execution/src/terminal_holder.rs` |
| `transport/held.rs`, `transport/hosted.rs`, `transport.rs` | same names under `rust/loopflow-execution/src/` |

Revisions on main: holder protocol 2 (`terminal_holder/wire.rs`), viewer
entrance 4 (`transport/registry.rs`; was 3 at merge, advanced by later work).
All four behaviors are still called there: `finish_paste` on control loss
(`hosted.rs:283,607,774`), `input_paste.finish()` on holder writer loss
(`terminal_holder.rs:436`), `resume()` framing (`held.rs:281`,
`transport.rs:1654`), `detaches` (`transport.rs:2147,2225`).

## Outcome people see

Someone pastes into a Session, and the view drops mid-paste (Desktop or Rex
closes, another view takes control, the server restarts). On return, keys and
Return reach the provider's prompt. Before, the provider's parser stayed inside
the paste and every later key vanished into it until the Session was restarted.
Pasted text containing Ctrl+] no longer detaches the view.

## Acceptance check (verify step, from a checkout of origin/main)

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-execution --lib paste < /dev/null
```

Expected: the nine paste regressions pass, including
`an_interrupted_paste_ends_before_the_next_view_types` (disconnect, replace and
server restart against a raw PTY) and
`a_lost_writer_finishes_its_paste_without_submitting_it`. The package name
changed with #1954; confirm the filter still selects nine before reading the
result.

On the live Machine: every `lf holder run` process seen on 2026-11-21 started
on or after 2026-11-17, after the merge. The "revision 1 holders retain their
writer semantics" caveat from the brief therefore likely covers no live
conversation any more. Verify confirms by asking each holder its revision
(`Client::hello`), or accepts the start dates.

## Not established, and left out

- Which transition lost LOO-904's paste-end marker. The repair closes every
  path found (viewer leave, control change, detach, holder writer loss,
  reconnect discard); the original event was never recorded. A recurrence on a
  revision 2 holder would be new information and a new Task, with the holder
  ring and a direct `ESC [ 201 ~` probe as the first read
  (`~/.lf/reports/LOO-912-stuck-sessions/stuck-session-input.md` has the method).
- LOO-515's stall: the Session changed from Codex to Claude mid-inspection, and
  the same cause was not confirmed.
- No paste timeout, no added Return, no content replay. These were excluded in
  the design and stay excluded.

## What goes

Nothing in the product. After completion, this worktree and branch are
disposable; `~/.lf/reports/LOO-912-stuck-sessions/` holds the only record of
the live observations and stays.

## Concept review (2026-11-21, against origin/main `891d9a9b92`)

The paste model holds with the whole change in it: one `Paste` type counted at
each hop that can lose its upstream (relay, server queue, server-to-holder
Source, holder), closed exactly, never by a blanket `ESC[201~`. Counterexamples
tried and their outcomes are in `scratch/concept-review.html`. Nothing here
changes steps 1 and 2 below.

Separate outcomes found around it, each its own change on main, none filed:

- A. `Paste::finish` logs at debug with no terminal or cause, so a recurrence
  cannot tell "lf closed a paste" from "lf saw nothing". Move the line to the
  four callers at info with terminal and cause.
- B. `transport/README.md` says the entrance is "revision 3"; `registry.rs`
  says 4. Delete the number from the README.
- C. `closes_pastes = revision >= 2` (`held.rs`), its `[1, 2]` test loop and
  the README's "older holders" paragraph serve revision 1 holders. All thirteen
  holders alive here on 2026-11-21 started on or after 2026-11-17. Delete once
  each answers revision 2.
- D. `input::keys` strips report-shaped sequences from a non-sizing view even
  inside a paste, editing pasted content. Apply it only outside a paste.

Choice made for review: the merged Task is not reopened for these, and no Task
was filed for them; the page's question 1 decides.

## Remaining steps

1. Verify: run the acceptance check on main's code, and check the live holders.
2. `lf task complete LOO-912` once it passes.

Choice made for review: this plan treats the rerun of the Graph on a merged
Task as "verify and finish", not as a request for more paste work. Passed over:
planning a search for LOO-904's original loss event, which has no recorded
trace to start from.
