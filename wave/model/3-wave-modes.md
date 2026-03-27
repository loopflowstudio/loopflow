# Wave Modes — `flow` Replacing `manual`

**Finish line:** Wave mode `manual` is replaced by `flow`. `flow` means "started by something upstream — not self-initiating." `manual` is deprecated but treated as alias during transition.

## Context

Wave modes are now `manual` and `loop` (the `cron` variant was removed — scheduled work uses `crons:` entries instead of a mode). `manual` means "single run" but that's a misnomer — what it really means is "I don't self-initiate." The name implies a human pressed a button, but the wave could have been triggered by another wave, a cron entry, a webhook, or anything.

`flow` says what the wave actually is: part of a flow of work, started by something upstream.

## The change

| Mode | Who starts it | Lifecycle |
|------|--------------|-----------|
| `flow` | Parent wave, trigger, cron, or human | Runs to completion, stops |
| `loop` | Self | Runs continuously, pulls next item when done |

`flow` replaces `manual`. The wave doesn't self-initiate — something upstream starts it. That upstream could be a loop wave, a cron entry, a trigger, or a human running a command. The wave doesn't care which.

### Migration

Rename `manual` → `flow` in wave config parsing. Read `manual` as alias for `flow` during transition. Update all existing wave YAMLs that use `mode: manual`. Update `WaveMode` enum in Rust (`Loop | Flow`), Python, and Swift models.

## Done when

- `mode: flow` works in wave YAML
- `mode: manual` is accepted as alias, logs deprecation warning
- All existing `mode: manual` waves updated to `mode: flow`
- Wave mode enum in Rust/Python/Swift models updated
