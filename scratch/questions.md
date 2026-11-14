# LOO-715 unresolved choices

- November 14: The simplest reversible instrumentation keeps native disposal
  behavior unchanged while measuring it. The design requires a reproduced baseline
  before selecting a teardown workaround; a green model test cannot choose one.
- The harness's 500 ms retention bound is an experimental input, not a production
  policy or a proven safety barrier. Paint callbacks can be unavailable; two
  animation frames never count as proof of a committed layer tree.
- Jack Heart's production reporter decision remains open, but it does not block
  the isolated `-Dsentry=false` and envelope-correlation experiments (operator
  steer, November 14). Both experiments are now measured; the recommendation is in the plan.
  Headless review leaves the choice open. No reporter policy,
  installation, merge, OS upgrade or upload is included.
- November 14 measured counterexample to selecting a workaround: the final 48
  visible-window trials passed even with immediate release. Choosing a delay or
  blank navigation from that evidence would be unjustified. The missing evidence
  is a reproduced failing baseline / observed Desktop release trigger.

- November 14: newer trials exposed a missing display condition: console-session
  presence and a visible window flag did not imply unoccluded rendering.
  An 800×600 page was occluded and never supplied animation callbacks. The final
  probe reports unavailable; no additional survival count justifies a workaround.
  At realign the sole display reported `Display Asleep: Yes`; waking it needs a
  person at the Machine, not a harness change.
- Reversible experiment placement: the synthetic envelope candidate lives under
  `#[cfg(test)]` in `desktop_exits` until reporter review. Production integration
  would require executable and kernel process-birth fields currently absent from
  launch records. No production scan or fallback identity is inferred.
