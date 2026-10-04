# Decisions for review-design

October 4, 2026 — Jack Heart requested kickoff only. These are review questions,
not unanswered prerequisites for this headless planning run. Accepted direction and
required deletion scope are in [the design](focus-on-your-own-work.md).

1. **Review lifetime:** accept one ordinary sleeping Flow process at an interactive
   boundary, with its cursor only in memory? Recommended. Crash loses continuation;
   the caller inspects evidence and launches new work. Exiting every boundary would
   instead require newly authored segments or a durable continuation mechanism.
2. **Conversation delivery:** Claude stream-json is accepted, but current source
   does not establish review input in the retained native composer. Is adapting
   the existing conversation surface to that single owned driver acceptable if
   the native TUI cannot support it? No second provider or PTY injection is proposed.
   This is the material implementation/experience risk, not a request for SDK/hooks.
3. **Feedback result:** accept ordinary actionable feedback yielding a structured
   result in that same review turn, referencing Jack's actual input? Questions and
   ambiguous feedback stay discussion; no ready, complete, finish button or second
   acknowledgment. Baseline loop-decide consumes that result as its next input.
4. **Waiting:** accept immediate provider yield/pending-input signals plus the
   proposed 120-second quiet/no-outstanding-tools fallback, with one Waiting label?
5. **Legacy cutover and conversation lifecycle:** accept historical pending reviews
   retained as unresolved evidence with caller-owned recovery after a quiescent
   conversion? Removed Session Complete will also stop retiring ordinary
   conversations; process stop and Close view retain their distinct meanings.
6. **Optional looping direction:** prefer retaining loop-decide, or direct authored
   review-edge results with an autonomous decider retained for implementation loops?
   The design recommends the latter as the clearest alternative, but no option is
   accepted. Retain current templates until selected; this choice does not block
   the accepted conversation, Flow, Waiting or API-deletion work.

Routine proposals: preserve SQLite invocation/events as history, choose the existing
Task conversation under the scope lock (sole existing conversation, otherwise most
recently used), retain immutable captures, and use ordinary detached execution.
These are reversible implementation choices, not claims of Jack's approval.

Context accounting: `lf context --skill kickoff` reported local scratch and memory
within budget. The externally assembled Task seed plus retained steers totals
18,219 tokens against its 16,000-token goal allowance (2,219 over); all supplied
steers were read. Local document curation cannot shorten that external history.
No limit was raised and no Linear content was edited to hide the overage.
