# Live loopflow walkthrough

## Writing pass 1

Run ID: `run_2285e52a8e11406ba08f269a0e82c59a`.
Carried direction: none supplied for this initial writing pass.

Advance follows the Flow's forward edge to the next declared step, or finishes
the Flow when no steps remain. At an autonomous decision boundary, loop-decide
records Advance with supporting evidence through the typed Flow protocol. At a
human boundary, the human must explicitly authorize that exact gate; readiness
or provider exit does not approve it. Advancing one boundary leaves later human
gates intact and grants no delivery or merge authority.

This first pass deliberately covers only Advance, as required by the transport
fixture. The next writing pass must explain Iterate and Blocked and record its
distinct Run ID and the direction carried back to it.

The human's observation of the initial approval-to-worker handoff has not been
received through Ask. Leave that evidence pending for loop-decide to obtain
after the second writing pass. This artifact does not establish demo acceptance.
