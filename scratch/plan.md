# LOO-977: Codex usage limits move to the next agent

## Outcome

A headless Graph step on Codex that runs out of quota goes on under the next
`agents` entry instead of stopping the Graph. On 2026-11-18 LOO-974's
`implement` step (`codex:gpt-6.2-astra`) exited 1 with this line and the Graph
stopped, with `claude:fable` configured as the fallback:

```
codex_error: You’ve hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/settings/usage to purchase more credits or try again at 3:36 PM.
```

## The model

Why a turn failed is one fact. The provider states it; the harness decoder is
the one place that reads it; everything after works from `AgentFailure`.
Phrase lists are the fallback for a failure the provider gave no type for.

Today Codex's stated cause is dropped at `harness/codex_decode.rs` (every
error becomes `codex_error`) and guessed back from the English sentence in
`agent/mod.rs`. Claude already works the intended way: `rate_limit_event` →
`provider_rate_limited` → `AgentFailure::UsageLimit` (`harness/drive.rs:81`).

## What Codex says (read from this Machine, 2026-11-18)

`~/.codex/sessions/2026/**/rollout-*.jsonl` keeps each failed turn's error.
The 2026-11-18 failures, seven of them, read:

```
"error":{"message":"You’ve hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/settings/usage to purchase more credits or try again at 3:36 PM.","codex_error_info":"usage_limit_exceeded"}
```

Every typed failure in those files, 2026-10-06 to 2026-11-18:

| `codex_error_info` | count | message | text classifiers today |
|---|---|---|---|
| `usage_limit_exceeded` | 507 | `You’ve hit your usage limit. …` (always U+2019) | miss |
| `model_not_found` | 98 | `The 'gpt-6.1-astra' model is not supported when using Codex with a ChatGPT account.` | miss (`_model_unavailable` wants "is unavailable" / "is not available") |
| `server_overloaded` | 70 | `Selected model is at capacity. Please try a different model.` | Capacity |
| `unauthorized` | 37 | `Your access token could not be refreshed because …` | CredentialInvalidated |
| `other` | 12 | `workspace routing discovery failed`, `stream disconnected …` | varies |

Rollouts are Codex's core record, in snake_case. The app-server protocol lf
reads is camelCase: `codex app-server generate-ts` (0.170.0),
`v2/CodexErrorInfo.ts`: `"usageLimitExceeded" | "rateLimitExceeded" |
"serverOverloaded" | "modelNotFound" | …`, on `error` notifications and on
`turn/completed`'s `turn.error` (`harness/testdata/codex_error.jsonl`, where
it is `null`). Not captured: the app-server's own JSON for one of these
failures. Confirm the `unauthorized` spelling against `generate-ts` before
writing the table.

## Design

All in `loopflow-execution`.

1. **The decoder reads Codex's cause.** One function in `codex_decode.rs`
   turns an error object's `codexErrorInfo` into the event code, used by both
   places that build a terminal `ConversationEvent::Error` (the `error`
   notification without `willRetry`, and a failed `turn/completed`):

   | `codexErrorInfo` | code | `AgentFailure` |
   |---|---|---|
   | `usageLimitExceeded` | `provider_rate_limited` | `UsageLimit` |
   | `modelNotFound` | `provider_model_unavailable` | `ModelUnavailable` |
   | `unauthorized` | `provider_credential_invalidated` | `CredentialInvalidated` |
   | `serverOverloaded` | `provider_capacity` | `Capacity` |
   | null, missing, an object variant, anything else | `codex_error` | text decides |

2. **One table from code to failure.** `AgentFailure` gets the code → failure
   lookup beside its definition in `agent/mod.rs`. `drive.rs` replaces
   `limited: bool` with `failure: Option<AgentFailure>`: the first typed code
   seen, kept only when the exit is non-zero (the existing rule at
   `drive.rs:116`). Claude's `provider_rate_limited` goes through the same
   lookup. stderr reads `provider_rate_limited: You’ve hit your usage limit. …`.

3. **Provider words are folded once.** One private helper in `agent/mod.rs`
   lowercases a line and turns U+2018/U+2019 into `'`. The four classifiers
   that read provider words use it in place of their own `to_ascii_lowercase`:
   `subscription_limit_text`, `_model_unavailable`,
   `credential_invalidated_failure`, `classify_retryable_agent_failure`. The
   phrase lists stay ASCII. This covers a turn that fails with the sentence
   and no type.

Choices made for review:

- The Task brief asks only for the apostrophe match. The typed read is the
  main fix: the recorded failures carry the type, and the sentence came
  in four wordings in six weeks (two URLs, each with and without "Upgrade to
  Pro"). Passed over: the apostrophe match alone, which holds until the next
  rewording.
- All four observed kinds are mapped, not just the limit. It is the same read
  and the same table, and `model_not_found` (98 failures on 2026-11-13) is a
  second case the text misses today. Passed over: mapping only
  `usageLimitExceeded` and leaving the other three to phrase lists, which
  keeps two readers of one fact. `rateLimitExceeded` has never been observed
  and stays with the text.
- `unauthorized` moves the step to another provider, as both of its recorded
  messages already do through the text.

## What goes

- `drive.rs`'s `limited` flag and its one-code string compare.
- `ops/release.rs:2465-2471`, the second copy of the quota phrase list in
  `classify_release_notes_degradation` (also U+0027 only). It calls
  `subscription_limit_text` over the lines of its combined text and keeps only
  `provider usage limit`, which is lf's own wording
  (`AgentFailure::UsageLimit`'s display). `subscription_limit_text` becomes
  `pub(crate)`. `release_tests.rs:549` ("agent stopped after provider usage
  limit") must keep passing.
- The four per-classifier `to_ascii_lowercase` calls.
- The stale half of `subscription_limit_text`'s doc comment ("or the line a
  native Claude displayed"): its only caller reads a step's stderr.

Stays: every phrase now matched, and
`claude_lines_naming_no_spent_window_are_not_subscription_limits`.

## Absent and error states

- `codexErrorInfo` null, missing, an object variant or an unknown string:
  `codex_error`, as today; the text classifiers decide.
- `willRetry: true`: unchanged, no terminal error whatever the info says.
- A turn Codex finished after a typed error: no failure on exit 0.
- Two typed errors in one turn: the first is the failure.

## Acceptance check at gate

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-execution --lib -- agent:: harness::codex_decode:: < /dev/null
```

Passes, including:

- a decode test over a new `harness/testdata/codex_usage_limit.jsonl`
  (`codex_error.jsonl`'s shape with the recorded message and
  `codexErrorInfo: "usageLimitExceeded"`): one error, code
  `provider_rate_limited`; the existing `null` fixture still gives
  `codex_error`; one case per other mapped kind;
- a test holding the exact 2026-11-18 line under `codex_error:`, asserting
  `_classify_agent_failure` gives `Some(AgentFailure::UsageLimit)`.

Demo: a test in `rust/loopflow/tests/agent_tests.rs`, beside
`model_unavailable_result_falls_through_or_stops_without_spending_quota`,
where a Codex app-server stand-in (`support::codex_app_server_script`) answers
the turn with the recorded error and `claude:fable` is the fallback:
`run_agent` exits 0 with Claude's answer. It runs the decoder, `drive.rs` and
the recovery loop together, which no unit test does.

`task_graph_launch_tests.rs:1878` and `:2031` are not that demo: they insert
the reason string into `agent_limit_recoveries` and never classify it. They
stay as they are.

## Slices

1. `codex_decode.rs` reads `codexErrorInfo`; the code → failure lookup;
   `drive.rs`; the decode tests and the `agent_tests.rs` demo. Shows first
   whether the app-server payload carries what the rollouts do.
2. The exact-line unit test, the folding helper, the four classifiers.
3. `release.rs` reads `subscription_limit_text`.

## Exclusions

- Reading the reset time ("try again at 3:36 PM") from Codex's line.
- Native (interactive) Codex Sessions; this is the headless step path.
- Other harnesses' decoders.
- A typed failure on `CommandError`, so `release.rs` stops reading words.
