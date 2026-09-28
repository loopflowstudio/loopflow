# Session opening: kickoff evidence

Collected 2026-09-28 at repository `a11a6567d`, Claude Code `2.1.284`.
The [design](session-opening.md) owns the proposed implementation; the
[prior analysis](session-opening-incident.md) retains the incident observations.

## Sources and local constraints

- [Claude session documentation](https://code.claude.com/docs/en/sessions#where-transcripts-are-stored)
  describes per-project JSONL storage, configurable config/project directories,
  retention and suppressed persistence. It explicitly calls the entry format
  internal. Consequently, neither a fixed cwd-derived path nor a general JSONL
  parser is a complete provider contract.
- [SDK session documentation](https://code.claude.com/docs/en/agent-sdk/sessions#resume-across-hosts)
  describes cross-directory lookup on recent CLI versions and the need for local
  transcript files when resuming. These claims informed probes, not assumptions
  of universal loader behavior.
- [Anthropic's Python SDK reader](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/sessions.py)
  catches `OSError` in `_try_read_session_file` and returns `None`;
  `_resolve_session_file_path` also suppresses stat errors. Its convenience
  absence result cannot authorize replacement when access failure must remain
  distinct. No SDK dependency was installed.

## Installed CLI loader probes

Fresh temporary config and workspace directories were used. Subprocesses received
only PATH/HOME/USER/TMPDIR from the parent plus their disposable
`CLAUDE_CONFIG_DIR`; no account credential or dotfile was copied. Tools and MCP
were disabled and no model response was obtained. Logs stayed in the disposable
directories; the tracked [receipt](session-opening-probes.json) contains only
case names, UUIDs, statuses, paths, and boolean/size observations.

Common invocation (values varied by case):

```bash
CLAUDE_CONFIG_DIR=<disposable-account> claude -p \
  --setting-sources '' --strict-mcp-config --mcp-config '{"mcpServers":{}}' \
  --tools '' --resume <uuid> /exit
```

Important limitation: in this print-mode environment `/exit` produces
`/exit isn't available in this environment.` A return code of 0 proves the
loader accepted the transcript and reached local-command handling. It does not
prove that an interactive terminal opened or that a model answered a seeded
conversation. Claude itself persisted local-command records during these calls.

| Input before resume | Observed result |
| --- | --- |
| No matching file | Exit 1, `No conversation found`. |
| Zero-byte file | Exit 1, same error. |
| Valid file-history snapshot only | Exit 1, same error despite a nonempty file. |
| Complete synthetic user record with UUID/session ID/timestamp/cwd/version | Exit 0, reached local-command handling. |
| User plus a minimal synthetic assistant record | Exit 1, missing-conversation error. |
| Same assistant fixture, adding timestamp/cwd/version | Exit 0. This changed three fields; it does not isolate which individual field was necessary. |
| Truncated user record only | Exit 1, missing-conversation error. |
| Complete user record followed by truncated assistant record | Exit 0. Trailing partial data does not erase all saved history. |
| Fresh native `/exit` print-mode launch, then resume its generated transcript | Both return 0; transcript has system `local_command` entries with UUID/timestamp/content, no user/assistant exchange required. |
| Native command-only transcript placed in another project directory | Exit 1 despite file presence; same-directory copy loads. |
| User-containing transcript placed in another project directory | Exit 0. Cross-directory lookup is real, but the command-only counterexample remains. |
| Fresh native empty prompt | Exit 1 with `Not logged in`; it still writes history. Subsequent `/exit` resume returns 0. |

The synthetic user/assistant cases test the **real installed loader** on
constructed input. They do not prove how Claude writes an ordinary conversation.
The native local-command and empty-prompt cases test provider-written files, but
not authenticated model interaction. No fixture result justifies deleting an
equivalent existing file.

## Premature-exit lifecycle probe

In a second disposable directory, start the same isolated CLI with no input:

```bash
claude -p --input-format stream-json --output-format stream-json --verbose \
  --session-id <uuid> --setting-sources '' --strict-mcp-config \
  --mcp-config '{"mcpServers":{}}' --tools ''
```

The supervising Python process kept stdin open, observed the process still alive
after 0.5 seconds, sent SIGTERM, then waited. It exited 143 with no matching
history. Native resume returned 1/missing conversation. A fresh print-mode local
command with the same allocated ID returned 0 and wrote native history; native
resume then returned 0 and reached local-command handling.

This distinguishes ID allocation from persistence without guessing the original
incident's exit cause. It is not `lf session open`, does not include a review
seed, and does not satisfy the design's real TUI acceptance criterion.

## Source findings

- `lf/commands/util.rs::session_command_status_with_env` records the ID/account
  before spawn; `lf/commands/run.rs::launch_prompt` reads it back and observes it.
- `ops/human_session.rs::session_run_is_resumable` tests receipt plus owned client.
  `spawn_session_run` uses `kill_on_drop` and a 30s deadline.
- `resume_native_run` stops clients before reading the reference. Stop and
  subsequent spawn acquire the provider lock separately, leaving a handoff gap.
- Ask/Task `open_boundary` clears readiness for replacement. Standalone
  `recover_unpublished_run` does likewise, treats any reference as published,
  and runs before the metadata-only-open return.
- `set_flow_position_in` conditionally updates the position, but writes
  `worker_generation=0` and requires `claim_json IS NULL`. A recovery update
  cannot use it while promising preservation of all execution fields.
  `validate_flow_position` also requires a claim's embedded `position_version`
  to equal the position's version. The final audit corrected the draft:
  rebinding cannot increment underneath an unchanged nonempty claim. The claim
  API also rejects human positions, so retain existing unclaimed-position
  ordering rather than inventing a claim-transfer or waiting protocol.
- Replacements rebuild current prompts; fresh interactive account selection is
  separate from pinned resume. `built.prompt` differs from the captured
  headless-oriented system/task channels. Fixing only history detection would
  not preserve the context/account promised by the incident prevention.
- `ProviderAccountRoute::apply` gives local/direct Claude a config directory,
  but a forwarded lease an OAuth environment value with ambient storage. Do not
  derive every history path from account ID. Inspection must not invoke secret
  extraction, credential refresh, or directory-creating account resolution.

All three original incident Run artifact sets still match their recorded SHA256
values; all three restored native transcript paths still exist. Only hashes and
existence were checked, with no provider launch against those reviews.

## Review findings applied

The review rejected nonempty-file and user/assistant-count persistence predicates.
It also rejected using the general Task position setter, rebuilding the seed,
silently rerouting a replacement, and extending only Ask/Task while leaving
standalone Flow stranded. The design now states exact lock boundaries, pending
startup behavior, archived-context limitations, and forbidden replacements.

No Rust/Python production code, account authentication, installed binary, review
binding, readiness or feedback was changed. No source test suite was rerun for
this documentation pass. All disposable probe processes exited.
