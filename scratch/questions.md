# Open design decisions

1. An OpenCode wake through `session.synthetic` shows the answer in the TUI
   but not the steer that caused it; `session.prompt` shows the steer as if
   the person typed it. The plan uses `synthetic`, as Claude's inbox
   attributes the message to lf. Is an answer with no visible cause
   acceptable in the terminal?

2. Claude's idle wake reads Claude's session record and inbox socket. Is
   that format documented? If it changes, the steer waits for the next turn.

3. GUI chat in Desktop and Mobile. Jack Heart asked, “if you want to
   convert an existing temrinal chat, does that require starting a new
   agent process regardless and (b) does that make us want to bring back
   more server style code”, and what other orchestrators do. Research is
   separate from this Task's build.

4. An OpenCode turn that fails or is interrupted reports no turn end, so the
   work session stays "in a turn" and a steer waits for the next one. Claude
   behaves the same after an interrupt. Reporting `session.execution.failed`
   and `.interrupted` as ends would wake it sooner, and would also start a
   turn right after someone pressed Esc. Left as is.

5. The wake socket sits in a `0700` directory under `/tmp` and takes no key;
   Claude's inbox takes one. Is directory permission enough, or should the
   boundary carry a token the server sends back?

6. A newly reachable provider is recorded as idle whatever the earlier one on
   that Session was doing. Moves to LOO-1068 with the reopen rule.

7. A reopened OpenCode session keeps the model of its last turn. Answered 4b;
   built as `session.switchModel` in the launch's first `prompt` hook. Not
   probed: whether an idle wake (`session.synthetic`) passes through that
   hook. If not, a reopened work session whose first turn is a wake answers
   on the earlier model until someone types.

8. The boundary's `inbox` changed shape for Claude as well. An `lf` hook and
   server from different builds disagree on it, and the boundary is then a
   recording gap; nothing in the hook path stays readable across the change.
