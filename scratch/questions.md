# Decisions for review-design

October 4, 2026 — Jack Heart requested kickoff only. These are review questions,
not unanswered prerequisites for this headless planning run. Accepted direction and
required deletion scope are in [the design](focus-on-your-own-work.md).

1. **Segment API — selected direction:** Jack Heart requested run-until-human and
   start-at-step APIs, resilient to deep nested loops and repeated skill names.
   Sleeping runners and replacement composers are superseded. Exact spelling and
   immutable position representation remain implementation proposals.
2. **Entry semantics:** distinguish starting at a node from continuing after a
   reviewed occurrence. Preserve captured branch and nested loop state; a skill
   name alone cannot address either. Arbitrary entry must expose skipped work.
3. **Feedback and duplicate launches:** define how the native conversation agent
   supplies authored feedback with an exact boundary reference through ordinary lf.
   Idempotent explicit launch receipts must prevent duplicate effects without
   restoring worker claims or automatic recovery. No ready/complete handshake.
4. **Waiting:** accept immediate provider yield/pending-input signals plus the
   proposed 120-second quiet/no-outstanding-tools fallback, with one Waiting label?
5. **Legacy cutover and conversation lifecycle:** accept historical pending reviews
   retained as unresolved evidence with caller-owned recovery after a quiescent
   conversion? Removed Session Complete will also stop retiring ordinary
   conversations; process stop and Close view retain their distinct meanings.
6. **Looping split — selected direction:** Jack Heart confirmed distinct
   conversational review and autonomous decisions, and requested renaming the
   autonomous loop-decide skill to loop-or-next. XORs and loops each support human
   or autonomous decisions. The native Task Session interprets conversational human
   choices and executes authored edges. Annotation syntax and segment feedback
   representation remain draft; no separate approval handshake.
7. **Loop syntax:** Jack confirmed the decider runs last after the body. An
   explicit loop node with a backward pointer is preferred initially; top-declared
   regions remain an alternative Jack is open to. Settle canonical YAML while
   preserving shared return targets, nested occurrence identity and minimal naming.

Routine proposals: preserve SQLite invocation/events as history, choose the existing
Task conversation under the scope lock (sole existing conversation, otherwise most
recently used), retain immutable captures, and use ordinary detached execution.
These are reversible implementation choices, not claims of Jack's approval.

Context accounting: `lf context --skill kickoff` reported local scratch and memory
within budget. The externally assembled Task seed plus retained steers totals
18,219 tokens against its 16,000-token goal allowance (2,219 over); all supplied
steers were read. Local document curation cannot shorten that external history.
No limit was raised and no Linear content was edited to hide the overage.
