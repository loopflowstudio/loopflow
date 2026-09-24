# Iteration 9 — configured terminal viewport retention

The signed current integration bundle preserves a genuinely scrolled terminal
through Work navigation. No production or test source changed. Session-row
return and nested checkout proof remain open after foreground checks stopped
those attempts before provider launch.

## Configured result

Used `/tmp/loo291-iteration8/Loopflow Proof.app`, executable SHA-256
`75fdef9e4d35fed6deeb01c3995bddcd5c7e0c6e5b7f88ce733aff01c4ae1abe`, with the
installed development CLI/Home explicitly aligned as in iteration 8. Signature
verification passes. This is the actual repository, with no fixture/capture
mode or installed-app replacement. The retired iteration 8 Session was not used.

The successful [probe](configured-ui-evidence/iteration9/wheel-active-probe.swift)
launched its own app, selected LOO-291, opened All work and New terminal, and
printed 200 numbered rows. Its [receipt](configured-ui-evidence/iteration9/wheel-uninterrupted.log)
records AX trust, one accessible window, activation, and successful controls.
A wheel event positioned inside that owned window scrolled actual history.
The cursor was restored afterward. The three screenshots were visually inspected:

- [Bottom](configured-ui-evidence/iteration9/wheel-bottom.png): rows 168–200,
  the bottom marker and shell prompt.
- [Scrolled](configured-ui-evidence/iteration9/wheel-scrolled.png): initial
  command and rows 1–33; the bottom output is outside the viewport.
- [Returned](configured-ui-evidence/iteration9/wheel-returned.png): the same
  rows 1–33 after Show work list → Work details → All work → Return to terminals
  → Hide work list.

This proves configured terminal viewport retention across those controls, not
repository return, nested checkout changes, or provider draft retention. The
latter has the separate successful iteration 8 receipt. Launch to observed Task
was 4,810.8 ms in this trial, including probe/setup overhead; it is not a pixel
paint measurement, cold-launch budget, or qualifying performance series.

## Failed attempts and exact boundary

The first `shell-probe.swift` delivered wheel events directly to the PID. Both
`bottom.png` and `scrolled.png` still show bottom rows; this is a failed scroll
attempt. The next wheel attempt and its activation follow-up stopped at an
inactive-app assertion. The same follow-up script then completed in one
uninterrupted tool call and produced the successful screenshots above. Different
event delivery established scrolling; why activation varies remains unproven.
Do not infer that tool polling caused the earlier failures.

Five separate Session/nested-layout attempts stopped before pressing New
conversation. Early attempts reported inactive; later diagnostics showed
`NSRunningApplication.isActive` and `NSWorkspace.frontmostApplication` disagreeing.
The final probe read Accessibility's focused application directly: owned app
PID 72044 was not focused; Warp PID 33576 was. AX trust was true and the desktop
had no locked-screen flag. No additional permission or unlock is indicated.
The last AX tree also contained search text `there`, which hid the target Task;
its origin is unknown. That attempt reached neither exact Task selection nor
provider launch. These are observation failures, not Session or nested-layout
verdicts. No more foreground takeover attempts followed that diagnosis.

All scripts/logs are retained under `configured-ui-evidence/iteration9/`.
They are historical executed probes, not reusable test commands. A future trial
must allocate a fresh receipt directory, verify the exact owned focus and Task
control before mutation, and recover any exact owned Session before retrying.

## Review and remaining work

Reviewed the configured route against the existing owners: AppKit wheel events
enter Ghostty's existing scroll method; Work navigation retains the same native
view through the window-local pool. No alternative scroll controller, terminal
instrumentation, Session policy, DTO, or persistence was added. The screenshots
supply the missing visual consequence that unchanged bottom captures could not.

[Final receipt](configured-ui-evidence/iteration9/receipt.json) records unchanged
production/test hashes, all nine owned app PIDs absent, the existing sole Session
unchanged in the shared inventory, and LOO-291 incomplete at
2026-09-24T04:15:06Z. No provider was launched, moved, or completed in this pass.
No new test run or broad gate was warranted by a source change.

Next proof remains exact Session-row return into its existing shell and two
checkout groups retaining independent inner layouts/processes, on a foreground
available to the owned runner. Other scopes and external launch destination need
separate trials. Full LOO-291 still includes shared LOO-284 actions/display path,
Task-directive editing, human-selected external trials, and published budgets
with the required sample sizes. No publication, landing, Task completion or PM
write occurred. The prior resource-gate failure remains historical, not retried.
