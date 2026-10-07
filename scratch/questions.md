# LOO-398 assumptions and blockers — October 7, 2026

- Jack Heart's header steer is interpreted as the workspace breadcrumb bar;
  Task-page header intent remains a review judgment. Current panes and focused
  header both use literal, display-sanitized report text.
- Live writing is unfinished. The independent tty sink failed the no-downside
  condition: a PTY short-wrote 62/74 bytes and existing output has no shared owner
  to finish the escape before ordinary text. The revised cut must establish
  output ownership and explicit Flow-step sink inheritance before emission.
- LOO-394 owns the absent relay and its Codex/shell-restoration choices. Jack
  explicitly selected relay-side reading as a follow-up, not a PR prerequisite.
- LOO-384 remains for non-reporters until a supported Claude release proves
  complete reporting. Protocol adoption and installed app acceptance are unproved.
