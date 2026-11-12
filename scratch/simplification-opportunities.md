# Simplification: `lf …` to a running harness

Requested by Jack Heart, 2026-11-11: "this needs to be simplifeid and unified. I
wonder if there is a larger reduction to be done to make sure that teh codeflow and
datastructures and functions map today's paradigms rather than being retrofitted
onto yesterdays", then "i just think you should /reduce the code flow from `lf ...`
to launching a harness". The full analysis, read on LOO-823's `8e1c71d6d5`, is at
`43c6621de1:scratch/simplification-opportunities.md`.

## Done on this branch

Opportunity 2 (one `Holder`, one decider in `transport::delegate`) and
Opportunity 3 for bare Claude, Pi, OpenCode and Codex: see `native-provider.md`
"Built". Both earlier open choices were settled in code, reversibly: a headless
`--task` run is `Holder::Server` (every hook), and a bare launch without a tty on
stdin (an agent's Bash tool) keeps an owner PTY (`runs_in_terminal`).

## Remaining elsewhere

- Opportunity 1, LOO-824: the owner re-runs `lf <same args>` in its PTY
  (`transport::serve`) and hands the exit back (`hand_exit_record`). The holder
  should take a prepared Session (id plus provider argv), as
  `delegate_prepared_session` does for resume, leaving one delegation call and no
  second dispatch.
- Observers (`__codex-observe`, app-server) only for `Holder::Server`: owner-held
  bare Codex (Desktop shell) still starts them; one-line follow-up once the
  Desktop handshake goes (server-first "Every view types").
