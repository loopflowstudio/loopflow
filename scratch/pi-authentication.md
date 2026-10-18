# Pi authentication — 2026-10-17

Jack Heart selected OpenCode subscriptions only for his personal Pi setup and
configured-account checks. This is personal preference, explicitly not a
Loopflow product constraint. Preserve general Pi provider/model support; do not
test other account routes or introduce an OpenCode-only restriction.

Jack Heart asked whether existing subscriptions work with Pi and challenged the
claim that Claude permits it. Pi implementing an OAuth login is not evidence of
provider authorization or coverage by a subscription's included allowance.

Anthropic's current [login policy](https://support.claude.com/en/articles/13189465-log-in-to-your-claude-account)
reserves included usage for native applications, prefers API keys for third-party
tools, and allows certain third-party access at its discretion with paid usage
credits. No Pi-specific approval or approval date was established. Do not describe
Claude Pro/Max as a verified included-subscription route for Pi.

Pi [added its extra-usage warning](https://github.com/earendil-works/pi/commit/96916f2cadd9c3256dbfe8d65e29e47f2f3d3e74)
on April 8, 2026: third-party subscription login was billed per token through
extra usage. This dates Pi's warning, not Anthropic approval.

Pi 1.3.0 was installed globally after the cmux shim reported no available agent;
both the shim and the underlying executable returned version 1.3.0. No account
was authenticated or credentials copied. Pi owns login and model settings.

Jack Heart subsequently authenticated Pi himself and selected
`opencode/gemini-4-argon`. An ephemeral, tools-disabled request through that exact
route returned `PI_GEMINI_OK` with stop reason `stop`. The older
`opencode/gemini-3-flash` returned model-access-disabled; availability in the model
catalog is not account entitlement. Pi's settings API saved the selected Argon
model as the personal global default and read it back. No Loopflow configuration
or provider restrictions were changed. This proves direct Pi account access,
not yet the candidate Loopflow integration with that account.

After sync recovery, the candidate `target/debug/lf` also passed a headless
`connect --fresh` request with `pi:opencode/gemini-4-argon`: exit 0 and the expected
`PI_LOOPFLOW_OK` response. The check used a disposable Git checkout and LF_HOME,
disabled Pi tools and discovered extensions, and let Pi use its own login without
reading or copying credentials. Rendered interaction remains unreviewed.
