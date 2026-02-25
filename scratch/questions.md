# Open Questions

## Cross-repo sequencing for Studio auth

`loopflow.remote` and `../studio` both own pieces of the Studio-auth path. Do we want one integration owner per iteration (single PR plan spanning both repos), or keep independent tracks and sync at milestone checkpoints?

## Deployment UX

- Do we expose `executor.agent_timeout` in Concerto connection settings, or keep it daemon-config only in v1?
- Should remote capability warnings live in wave detail only, or also in wave edit/config surfaces?
- Connection settings now show Bundled/Remote mode. When studio auth ships, does "Remote" split into "Remote (static token)" and "Remote (studio auth)", or does studio auth replace static token for the Concerto UI path?
