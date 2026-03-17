# Clear the Deck

Cut surface area. Every item here is a deletion or a collapse — reducing the number of things that drain attention so the hard design work has room.

These are decisions, not design problems.

## Strategy

Ship the easiest, most reversible cuts first. Each PR stands alone. No item here requires design — the decisions are already made.

## Auth consolidation

Delete the studio auth service. Move WorkOS OAuth into lfd — PKCE, JWT issuance, user identity. Three modes:

```
Solo:   local token (auto-generated, current behavior)
Team:   WorkOS OAuth built into lfd
CI:     static token via LFD_AUTH_TOKEN
```

## Deployment collapse

Stop letting users modulate auth x storage x isolation x agent independently. Three blessed configs:

```
Solo:   local lfd + local agents + file-based state
Team:   shared lfd + auth + postgres + container isolation
CI:     headless lfd + single-run mode + no persistence
```

## Goals

- Studio auth service deleted, auth consolidated into lfd
- Deployment configs collapsed from combinatorial matrix to three blessed modes
- Custom sandbox code removed, Daytona evaluated
- Growth/marketing infrastructure deleted

## Risks

- Auth migration could break existing team deployments if not careful about the WorkOS transition
- Daytona evaluation might reveal it doesn't meet our needs, requiring a plan B for isolation

## Metrics

- Lines of code deleted (target: net negative across all PRs)
- Number of deployment configurations supported (target: 3, down from N)
- Time to onboard a new deployment mode (target: <10 minutes)
