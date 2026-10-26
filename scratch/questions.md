# LOO-616 assumptions

- 2026-10-26 kickoff: the pre-main scrub uses the `ctor` dev-dependency rather than a hand-rolled `link_section`. It is reversible; fall back to the link section only if offline builds reject the new crate.
- 2026-10-26 kickoff: the stray row is deleted with SQL after `lf account disconnect`, because no account-delete command exists. Adding one is out of scope.
