# LOO-1126 open questions

- Pass 2 defaults `TERM_PROGRAM=ghostty` for held terminals started with none. Chosen over `FORCE_HYPERLINK=1` (leaks to non-TTY children) and a `loopflow` name (nothing recognizes it). Is naming the held libghostty-vt terminal "ghostty" acceptable?
- iPhone Sessions: links don't open either (action unhandled, Ghostty's iOS opener unimplemented, taps carry no ⌘). Needs a touch gesture; left out. Worth its own Task?
- File links open in the default app, as in Rex. Should a file inside the Session's checkout open in Desktop's Files pane at its line instead?
- Claude's `file://` link shape (line fragment or not) was not confirmed from its binary; the policy drops an `#L<n>` fragment and refuses other fragments.
- Codex's hyperlink detection was not probed.
