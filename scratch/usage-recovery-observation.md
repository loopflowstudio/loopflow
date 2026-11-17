# LOO-880 recovery, 2026-11-13

Status inspection after the delivery handoff showed 27 stopped `deliver` Graphs: the first stopped at landing at 12:13:58 UTC; subsequent attempts stopped at gate, through 15:15:16 UTC. The status reason was: "The 'gpt-6.1-astra' model is not supported when using Codex with a ChatGPT account."

The first Graph log reported PR copy generation failure, then three failed attempts. Later status records showed more groups of attempts. The Task was still scheduling another retry. `lf task interrupt LOO-880` was issued to stop its scheduled automation. No PR existed on GitHub for the branch at inspection.

The first gate run reported 109 passing focused checks, formatting, Clippy and package verification. Landing was retried directly with prepared PR copy, avoiding another gate or model-generated description.

Prevention must cover automatic recovery as well as successful backward edges: a retry allowance must survive carrier/Graph restarts, and permanent configuration errors must stop rather than renew attempts. This is distinct from the proposed three backward-traversal allowance.
