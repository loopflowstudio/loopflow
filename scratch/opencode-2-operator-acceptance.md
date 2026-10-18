# Independent configured-account acceptance — October 17, 2026

Jack Heart requested getting OpenCode 2 running. The repository operator tested the LOO-504 candidate directly in a real PTY, separately from the implementation worker's deterministic/native probes. Candidate reported lf 0.14.5+28dc14167.dirty; installed OpenCode is 2.0.32. All inherited LF_* variables were removed, then only the disposable LF_HOME and explicit candidate LF_BIN were supplied. Provider HOME/config/account were retained, with no credential reads or account changes. The native provider selected opencode/claude-haiku-5-5.

Observed passes:
- Actual bare candidate `lf -a opencode` opened the native full-screen terminal, produced its initial answer and accepted editable input.
- Typed a request to run `printf opencode-2-native-ready` without file edits. The native shell completed with exit 0 and the model returned `OPENCODE_2_READY`.
- `/exit` closed normally with exit 0. The native continuation ID was displayed.
- Ran candidate `lf connect session_af73b8f4d292427f92a2514af6740b28 --prepare --json` in the disposable store; executed returned open_argv unchanged with inherited LF_* removed. The same native conversation and previous text/tool output reopened.
- Asked for the previous readiness token without supplying it or allowing tools. The model returned `OPENCODE_2_READY`. A second `/exit` completed with exit 0.
- `lf session history --json` retains all three turn completions and four unique native usage-message IDs. Old turns retain Process 456d0402-79ad-4984-af88-87a33f17a3f7; the post-resume turn belongs to 93722655-6533-41d1-b791-42fbba4f6557. All use native Session ses_eb36fa0b1ffeq36pJ5rbvsNa5B. The proof is docs/reviews/opencode-2/operator-native-results.json, filtered to exclude provider reasoning signatures.

Concrete repair finding: successful completion payloads currently include an `error` saying the execution ended without a successful answer even though `status` is completed. Do not carry that misleading fallback into successful receipts; add a focused assertion while retaining failure evidence.

Limits: this is an intermediate candidate, not the final gate or installed acceptance. No deliberate failure/resize or crash recovery was tested here. The initial source-CLI launch without explicit LF_BIN dispatched to the old installed adapter, so that failure is not candidate evidence. Explicit candidate launch against the shared store then failed before provider startup on missing transport_lf_process_id; source includes the preexisting session_transport draft. No shared-schema repair/migration was attempted. Final supported installed acceptance requires the normal release/materialization/install path, not applying a branch draft to the live store. Fixture setup also required an initial Git commit and clearing all inherited LF_*; those setup failures do not establish provider defects.

Fixture and retained receipts: /var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-opencode-operator-9bvgpzjn. Both native terminals exited normally; preserve evidence until delivery. This small configured-account test reports native usage costs totaling approximately $0.00692, independent of all MTG budgets.
