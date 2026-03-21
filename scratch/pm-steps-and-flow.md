# PM sync steps and flow — validation

## Try it

```bash
# Export pushes local state to remote
lf ops pm export pm           # push one wave
lf ops pm export --all        # push all PM-enabled waves

# Steps are discoverable
lf import-pm                  # runs pm pull for current wave
lf export-pm                  # runs pm export for current wave

# Flow chains the full cycle
lf pm-sync                    # import → implement → export

# Verification
cargo test -p loopflow pm_export    # unit tests pass
cargo clippy -- -D warnings         # no warnings
```

## Measure

Before: `lf ops pm` subcommands are `init`, `import`, `sync`, `pull`, `status`. No step/flow wrappers.

After: `lf ops pm export` joins the CLI. `lf import-pm`, `lf export-pm` appear as steps. `lf pm-sync` appears as a flow. Round-trip test: create item locally without provider ID → `lf ops pm export` → item appears in Linear with ID written back to frontmatter.
