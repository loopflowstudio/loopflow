# LOO-912 — verify interrupted terminal pastes

PR #1944 merged as `19872301e3`. This checkout now includes `origin/main`
at `891d9a9b92` and matches its product files. Verification can run here.

1. Run the paste regressions with the launcher's environment cleared and stdin closed.

   ```bash
   env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
     cargo test -p loopflow-execution --lib paste < /dev/null
   ```

   Expect nine tests. Check that `an_interrupted_paste_ends_before_the_next_view_types`
   covers disconnect, replacement and server restart against a raw PTY, and that
   `a_lost_writer_finishes_its_paste_without_submitting_it` passes.

2. Check the live holders. On November 21 all thirteen had started on or after
   November 17, after the November 15 merge. The installed `lf 0.16.0+a4589ee2a`
   contained the fix. Start dates are sufficient for this check. A holder's
   `Client::hello` reply gives its protocol revision directly.

3. Once both checks pass, run `lf task complete LOO-912`.

The original LOO-904 loss event was never recorded. LOO-515 changed from
Codex to Claude during inspection and its cause remains unknown. Live
observations are in `~/.lf/reports/LOO-912-stuck-sessions/stuck-session-input.md`.
The [concept review](concept-review.html) records the counterexamples and four
possible follow-ups.
