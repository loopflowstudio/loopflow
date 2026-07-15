# 5 Whys: Claude OAuth showed a paste code

## The Problem

`lf auth connect claude --account primary` printed an `Enter code` value even
though Claude completes browser authorization through its callback, then left
the provider login running when Loopflow was interrupted.

## Chain

False device code → URL-wide regex → provider flows shared an untyped parser →
tests used synthetic device URLs → auth subprocess ownership ended at a detached
task instead of the CLI process.

**Problem**: Loopflow displayed Claude's OAuth client id as a user code.
↳ *The exact Claude authorization URL was never a parser fixture.*

**Why 1**: The generic parser searched the entire URL for any hyphenated
alphanumeric value.
↳ *It should parse `user_code` only from an explicit query field and parse text
codes only after a provider asks for one.*

**Why 2**: Claude, Codex, and Doppler shared a parser shaped around device-code
output even though Claude's browser callback has no user code.
↳ *The flow response treated a user code as guessed text rather than typed
protocol evidence.*

**Why 3**: Parser tests used example URLs and ANSI snippets, not captured
provider-shaped frames.
↳ *A synthetic URL did not contain the UUID fields that trigger the false
positive.*

**Why 4**: Returning the URL was considered completion of auth startup, while
the subprocess continued inside a detached Tokio task.
↳ *Cancellation ownership was not represented in `AuthFlowHandle`.*

**Why 5 (Root)**: The auth abstraction modeled all browser logins as loosely
parsed console text and did not make either protocol evidence or child-process
ownership explicit.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 2 | Can each provider expose a structured auth-start response? | Medium |
| Why 4 | Should every long-lived child use one shared process-group owner? | Medium |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Ignore UUIDs and other values embedded in auth URLs | False Claude paste prompts |
| Structural | Accept codes only from explicit query fields or code-prompt text | URL metadata becoming user instructions |
| Structural | Make the auth monitor abort-on-drop and own a kill-on-drop process group | Orphaned OAuth children |
| Systemic | Test the exact Claude callback URL shape and cancellation behavior | Regression across provider CLI updates |

## Changes to Implement

- [x] Parse explicit `user_code` query parameters separately from console text.
- [x] Track when console output is actually asking for a code.
- [x] Make dropping an auth handle terminate its provider process group.
- [x] Add Claude callback and cancellation regression tests.
