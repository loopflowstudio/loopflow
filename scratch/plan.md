# LOO-977: Codex usage limits move to the next agent

## Outcome

A headless Graph step on Codex that runs out of quota goes on under the next
`agents` entry instead of stopping the Graph. On 2026-11-18 LOO-974's
`implement` step (`codex:gpt-6.2-astra`) exited 1 with this line and the Graph
stopped, with `claude:fable` configured as the fallback:

```
codex_error: You’ve hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/settings/usage to purchase more credits or try again at 3:36 PM.
```

## Why it stopped

- `subscription_limit_text` (`rust/loopflow-execution/src/agent/mod.rs:1352`)
  lowercases with `to_ascii_lowercase` and matches `you've hit your` with
  U+0027. Codex writes U+2019. No other phrase in the list occurs in the line
  ("usage limit reached" does not; `classify_retryable_agent_failure` wants
  "try again later", the line says "try again at"). `_classify_agent_failure`
  returns `None` and the step is a plain failure.
- Text is the only thing typing a Codex limit. `harness/drive.rs:81` sets
  `failure: UsageLimit` only for the error code `provider_rate_limited`, which
  only `harness/claude.rs:314` emits. `harness/codex_decode.rs` emits every
  Codex error as `codex_error` and never reads the payload's
  `error.codexErrorInfo`.
- Codex 0.170.0's protocol names the case. `codex app-server generate-ts`,
  `v2/CodexErrorInfo.ts`: `"usageLimitExceeded" | "rateLimitExceeded" |
  "serverOverloaded" | "modelNotFound" | …`. The field sits on both
  `error` notifications and `turn/completed`'s `turn.error` (see
  `harness/testdata/codex_error.jsonl`, where it is `null`).
- Not confirmed: that the 2026-11-18 failure carried
  `codexErrorInfo: "usageLimitExceeded"`. No raw payload of it is on this
  Machine (`~/.lf/logs` has none), and reproducing it needs a spent account.
  So the wording match stays and is what the acceptance check runs through.

## Design

Two changes, both in `loopflow-execution`.

1. **Provider words are folded once.** One private helper in `agent/mod.rs`
   lowercases a stderr line and turns U+2018/U+2019 into `'`. The four
   classifiers that read provider words use it in place of their own
   `to_ascii_lowercase`: `subscription_limit_text`, `_model_unavailable`,
   `credential_invalidated_failure`, `classify_retryable_agent_failure`.
   The phrase lists stay ASCII.
2. **Codex's own code types the limit.** In `codex_decode.rs`, both places
   that build a terminal `ConversationEvent::Error` (the `error` notification
   without `willRetry`, and a failed `turn/completed`) read `codexErrorInfo`;
   `"usageLimitExceeded"` gets the code `provider_rate_limited`, everything
   else stays `codex_error`. `drive.rs` then sets `AgentFailure::UsageLimit`
   with no dependence on wording, the same path Claude uses. stderr reads
   `provider_rate_limited: You’ve hit your usage limit. …`.

Choice made for review: (2) goes beyond the Task brief, which asks only for
the apostrophe match. It is included because the next rewording of the
message would stop Graphs the same way. Passed over: mapping
`rateLimitExceeded`, `serverOverloaded` and `modelNotFound` to typed failures
too; `provider_rate_limited` is the only code `drive.rs` types today, and the
text classifiers already cover those.

## What goes

- `ops/release.rs:2465-2471`, the second copy of the quota phrase list in
  `classify_release_notes_degradation` (also U+0027 only). It calls
  `subscription_limit_text` over the lines of its combined text and keeps only
  `provider usage limit`, which is lf's own wording
  (`AgentFailure::UsageLimit`'s display). `subscription_limit_text` becomes
  `pub(crate)`. `release_tests.rs:549` ("agent stopped after provider usage
  limit") must keep passing.
- The four per-classifier `to_ascii_lowercase` calls.

Stays: every phrase now matched, and
`claude_lines_naming_no_spent_window_are_not_subscription_limits`.

## Absent and error states

- `codexErrorInfo` null, missing, an object variant or an unknown string:
  `codex_error`, as today; the text classifiers decide.
- `willRetry: true`: unchanged, no terminal error whatever the info says.
- A turn Codex finished after a limit event: `drive.rs:116` already ignores
  the flag on exit 0.

## Acceptance check at gate

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-execution --lib -- agent:: harness::codex_decode:: < /dev/null
```

Passes, including:

- a test holding the exact 2026-11-18 line above, asserting
  `_classify_agent_failure` gives `Some(AgentFailure::UsageLimit)`;
- a decode test: an `error` notification and a failed `turn/completed` with
  `codexErrorInfo: "usageLimitExceeded"` each produce one error with code
  `provider_rate_limited`; with `null` the code is `codex_error`.

Demo: `rust/loopflow/tests/work/task_graph_launch_tests.rs:2031` already runs
a Graph whose Codex step fails with a usage-limit line and goes on under
claude. Its fixture line becomes the exact curly-apostrophe Codex line, so
`cargo test -p loopflow --lib task_graph_launch_tests::` shows the step move
on the wording that stopped LOO-974. CI runs that suite.

## Slices

1. The exact-line unit test (fails today), the folding helper, the four
   classifiers. This is the fix for the incident; it shows whether folding in
   one place disturbs any existing classifier test.
2. `codexErrorInfo` in `codex_decode.rs` with its decode tests.
3. `release.rs` reads `subscription_limit_text`; the Graph fixture takes the
   curly line.

## Exclusions

- Reading the reset time ("try again at 3:36 PM") from Codex's line.
- Native (interactive) Codex Sessions; this is the headless step path.
- Other harnesses' decoders.
