# Chord Review — 2026-03-20

## What happened

The proposed mutations (resequence chord-model, silence agent-embedding) were superseded by a broader restructuring during review.

### Proposed mutations — both rejected

**1. Resequence chord-model: area model before engine depth** — **Rejected.** The area model item was deleted entirely. Portfolio view and calibration view aren't actually blocked on it — they can read wave state the same way garden/scan does. Building the API before the client inverts the dependency. Let the UI drive the abstraction.

**2. Silence agent-embedding after items 04 and 01** — **Rejected.** The wave's priorities got reorganized directly instead. Portfolio and calibration views demoted to 4 (speculative). The wave goes naturally quiet after window composition ships — no forced silence needed.

### What happened instead

**Wave restructuring:**
- chord-model → model
- agent-embedding → macos
- dogfood → ios
- concerto deleted (items redistributed)
- trust deleted
- redesign → root (gardens all five children: model, macos, lfd, pm, ios)

**Priority reordering (model wave):**
- 1: wave modes (urgent)
- 2: planning flow, wave crons, concurrent ingest, wave data model → deleted
- 3: wave discovery, tend flow steps, vsm flow
- 4: letta, mutation API, DAG, API expansion (speculative)

**Item moves:**
- iOS TestFlight → ios wave as 1
- Concerto release UI → macos wave as 4
- Typed auth methods + OAuth-only PM auth → combined as pm/2-provider-auth
- Auto-send on silence → deleted
- Notion supporting docs → deleted
- Wave data model (chord-wave-area-model) → deleted

**Key decisions:**
- Concerto hosts terminal sessions for now; custom calibration UI comes later
- No API before client — portfolio view builds against raw data, abstraction emerges from use
- VSM flow may become a drum machine UI rather than a CLI command
- "Chord" as a separate concept may not be needed — waves with sub-waves are just waves

## Session Notes

Jack doesn't want speculative infrastructure ahead of demand. Build the client first, let the API shape emerge. The tend assessment was too conservative about dependencies — portfolio and calibration views aren't blocked on a formalized data model. The CLI tend flow is the right place to iterate on calibration UX before investing in Concerto UI for it.
