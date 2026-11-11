# Claude model and resilient preferences

Remaining: finish config and Graph command checks, Clippy and prompt goldens;
land this PR, install the new main build, then save personal `claude.model: opus`
and verify it through the installed CLI.

Jack Heart: “in this branch we should also make config reading robust to spurious unknown changes” and “you can just not set this until this branch hits main and then you can install the new lf and then add the key”.

The new key broke installed Graph reads when first saved; removing it restored
`lf graph show auto`. Keep it absent until the binary update. Known values stay
validated; unknown file preference keys are warned and skipped.

Local runs: 21 context_launch_tests passed; 17 config settings unit tests passed.
A real bare-Claude probe returned “Claude model default probe passed.” and its
Session record reported provider claude, model opus.

Fable overrides are commented out in personal config after “maybe comment out
those lines (make sure config parser ignores comments)”. Installed CLI accepts
the comments; kickoff/concept-review now inherit Codex. QA and task/wave/repo
session stay claude:opus. LOO-834 tracks Fable fallbacks and is unstarted.
LOO-832 is abandoned; this feature stays in PR 1886.

Final checks: cargo fmt, cargo clippy --all-targets -- -D warnings,
check_architecture.py and check_rust_test_inventory.py passed. The 121-test
nextest selection passed 120 (including goldens, documented commands, all
settings launch tests). Its only failure expected an unknown Wave file key to
be rejected; the test now expects that key to be skipped while wrong-scope
known keys still fail. Focused rerun pending. Production code was unchanged.
The corrected Wave config test passed on rerun. The full 121-test selection is
covered by 120 passing tests plus that focused rerun; all 21 context launch
and resume tests also passed. Clippy is being repeated after the test edit.
