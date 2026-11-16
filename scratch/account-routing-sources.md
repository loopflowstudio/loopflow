# Account routing comparison — 2026-11-15

Read-only source review, no live provider or compatibility tests. Local HEAD:
`9cafc410eeab03d772b8cac7d641d6bd7edfd435`.
CLIProxyAPI main resolved to `43806c865da7d47f9252316e95e315ad9f885b84`.

External facts at that revision:

- [Configuration](https://github.com/router-for-me/CLIProxyAPI/blob/43806c865da7d47f9252316e95e315ad9f885b84/config.example.yaml): round-robin, weighted-round-robin, fill-first; configurable affinity and rebinding; retry rounds and cooldowns. Affinity defaults off; model-level cooling defaults off for Claude and Codex. Usage aggregation defaults off; management usage queue retention defaults to 60 seconds. Weighted credits are scheduling counters, not provider usage credits. Claude and Codex protocol adaptation includes configurable cloaking.
- [Quota observations](https://github.com/router-for-me/CLIProxyAPI/blob/43806c865da7d47f9252316e95e315ad9f885b84/sdk/cliproxy/auth/quota_signals.go): passive provider response signals, distinct from scheduling state. Captures Claude unified limits and Codex plan, credit, and window headers.
- [Usage records](https://github.com/router-for-me/CLIProxyAPI/blob/43806c865da7d47f9252316e95e315ad9f885b84/sdk/cliproxy/usage/manager.go): request/trace/selection/session/parent IDs, AuthID/AuthIndex, provider/model, token detail, latency, failure, quota headers. These supply account attribution for traffic crossing the proxy, not an lf Task/skill mapping or durable billing ledger by themselves.
- [Browser opening](https://github.com/router-for-me/CLIProxyAPI/blob/43806c865da7d47f9252316e95e315ad9f885b84/internal/browser/browser.go) and [login options](https://github.com/router-for-me/CLIProxyAPI/blob/43806c865da7d47f9252316e95e315ad9f885b84/sdk/auth/interfaces.go): default-browser OAuth callback flow with manual/no-browser options; inspected core has no saved per-account Chrome-profile selection. Management recovery checks target-account identity; this is not absent identity management.
- [Management auth view](https://github.com/router-for-me/CLIProxyAPI/blob/43806c865da7d47f9252316e95e315ad9f885b84/internal/api/handlers/management/auth_files.go): passive account/model quota views; v8 active quota support is exposed through quota-provider plugins. Plugin coverage was not tested.
- [Stream handling](https://github.com/router-for-me/CLIProxyAPI/blob/43806c865da7d47f9252316e95e315ad9f885b84/sdk/cliproxy/auth/conductor_stream.go): bootstrap buffering/retries and execution-result recording. No claim that arbitrary partial streamed responses can resume transparently across accounts.

Open validation: actual lf native-client compatibility; provider subscription behavior through custom endpoints; model-window and paid-credit parity; durable request/account joins under failover; relevant quota plugins and desktop browser integrations. Provider approval cannot be inferred from a component's packaging or popularity.
