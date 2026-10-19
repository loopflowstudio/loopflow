# LOO-426 assumptions — 2026-10-18

- Jack Heart accepted the clean repository/Wave/Task URI shape.
- Jack Heart clarified that `--available` opens conversations available for him
  to do his work with agents. No pending question, review or action owed is
  required. Use this frame in internal names, UI, docs and agent instructions;
  the full audit and accepted recommendations now live in
  `scratch/loo-426-open-any-task.md`. Task dependencies are separate facts.
- Ordinary opening preserves explicit attachment ownership. A retained draft
  survives same-destination reentry; another client's unsent draft is not migrated
  implicitly. Cross-client input requires the existing explicit move action.
- Native renderer activation, Rex installed behavior and the four-conversation
  comparison remain demo evidence. No display is required by headless gate.

- Jack Heart requested one subagent each to investigate installed cmux and
  Herdr link forwarding and report results. Rex probing is deferred for lack
  of access.

- Jack Heart prioritized simple architecture and understandable behavior over
  brittle host-specific code. The revised design proposes a useful generic
  opening path with small optional public-interface integrations, explicit
  per-activation context and no origin reconstruction. Probes established stale
  OS receiver environment across panes and hosts; Herdr's callback carries
  current context. cmux native clicking remains unproved.

- Jack Heart closed design review: agents run `lf open` themselves when opening
  is obviously next, using the shared dispatcher; host-specific skills load on
  demand and are follow-up work. Implementation authorized via `pursue`.

- Implementation pass (October 18): `session` and `diff` remain accepted query
  selectors on Task addresses so `lf open --session/--diff` can hand an explicit
  Session to Desktop; ordinary emitted links never include them. Simpler than a
  second Desktop channel; reversible.
- Desktop's own `loopflow://portfolio` and `loopflow://sessions` window links
  still win when the address has no path, so a repository folder named
  `portfolio` or `sessions` opens that window instead of its repository root.
- A live conversation (provider alive) in a terminal host is reported
  `elsewhere` with its `--takeover` command until destination receipts exist,
  matching `lf session connect`'s refusal rule. `--available` therefore opens
  new destinations only through Desktop today.
- Rex is accepted by `--host` but reports unsupported; automatic routing never
  selects it.

- Second implementation pass (October 18): the restored first pass conflicted
  with v0.14.7's palette Wave links (`loopflow://open?repo=&wave=`). Resolved by
  routing Waves through the typed `LinkedWork` delivery instead of keeping a
  second URL grammar; reversible.
- The Desktop-absent receiver installs automatically during `lf install promote`,
  following the native-hook precedent, rather than behind a new command. The
  Herdr link handler is linked the same way when `herdr` is on PATH. Both are
  warnings on failure. No uninstall command exists, so none was added.
- Receiver tests are Rust unit tests, not the planned pytest; the logic is Rust.
- `wave-report.md` still says "Needs <name>'s attention" in its hot-signal row.
  Left unchanged: not among the accepted operator-report edits, and possibly
  Jack Heart's own wording.

- Third pass (October 18): Desktop batch delivery assumes `open -b <id> u1 u2`
  hands Desktop each URL in order before any `lf open --link` read returns.
  SwiftUI cannot open a window without ordering it front, so an earlier item
  needing a new window shows briefly before the final item's window takes key.
  Demo confirms both natively.
- `available` takes ANSI magenta as its seed; per-theme colors await Jack
  Heart's visual review. Existing `loopflow-waiting-*` colors become warning,
  the role they mostly served.
- An unreadable execution owner now refuses opening (link printing still works),
  including shared Tasks whose first-start admission cannot be read.
- Compress (October 18): `--wave` with `--session` (or a Wave link with
  `--session`) now always checks the Session belongs to that Wave; a Wave link
  previously ignored the check. One resolver, `work_context`, owns it.
