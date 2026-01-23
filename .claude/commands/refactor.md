---
interactive: true
requires: existing code to restructure
produces: refactored code, updated design doc
---
Co-design a re-architecture of existing functionality.

## Goal

Refactoring is redesign with constraints. The current implementation works—users depend on it. The question isn't "what's ideal?" but "what's better that we can actually get to?"

A collaborative exploration. Map the territory, surface tradeoffs, propose directions. The human decides which constraints to keep and which to break. Find a path that improves structure without breaking what works.

## Workflow

1. **Scope the target**
   - If args were passed (e.g., `lf refactor: session tracking`), focus there
   - Otherwise, ask: "What feels wrong? What's hard to change?"
   - Read the relevant code and build a mental model

2. **Diagnose the friction**

   Before proposing solutions, name the structural problem:

   - **Scattered responsibility**: Logic for one concept spread across files
   - **Tangled concerns**: Two things that change for different reasons are coupled
   - **Wrong abstraction**: A class/function that doesn't match how it's actually used
   - **Missing concept**: Code works around something that should exist but doesn't
   - **Leaky boundaries**: Internal details exposed to callers

   State the diagnosis: "The friction is scattered responsibility—session state lives in three places and they drift out of sync."

3. **Map the constraints**

   What can't change?
   - CLI interface (flags, output format)
   - File formats (config, state files)
   - External APIs (what other code calls)

   What's expensive to change?
   - Data structures with many consumers
   - Patterns used throughout the codebase

   Ask about any constraints you're unsure of.

4. **Propose directions**

   Present 2-3 restructuring approaches that differ in *structure*, not just naming. Each should:
   - Address the diagnosed friction
   - Respect the hard constraints
   - Make clear what it costs (migration, breaking changes, complexity)

   Example:

   ```
   **Direction A:** Centralize session state in a Session dataclass
   - Pros: Single source of truth, explicit state
   - Cons: Need to thread Session through 4 call sites
   - Migration: Medium—change internal APIs, external unchanged

   **Direction B:** Make SessionStore a proper class with load/save
   - Pros: Encapsulates persistence, easier testing
   - Cons: More abstraction, class where functions might do
   - Migration: Low—wrap existing code
   ```

   Present tradeoffs clearly without picking. If unclear, ask what matters more: explicitness or convenience? Testability or simplicity?

5. **Iterate on the design**

   Based on their choice:
   - Sketch the key data structures in code
   - Show what the main call sites would look like
   - Surface any second-order decisions

   After each round, check: "Does this feel right? What's still bothering you?"

6. **Write incrementally**

   When direction is clear:
   - Make one structural change
   - Run all relevant tests (see TESTING.md for the full suite)
   - Commit if green
   - Repeat

   Don't batch up changes. Each commit should be a working state.

## Questions to ask

- "What's the most painful thing about the current code?"
- "If you could change one thing without consequences, what would it be?"
- "Does this direction feel like it's fighting the codebase or working with it?"
- "What am I missing about how this code is actually used?"

## Guardrails

**Preserve behavior.** Refactoring changes structure, not function. If behavior must change, make it a separate decision.

**Respect existing patterns.** The codebase has conventions. Work with them unless there's a strong reason to diverge.

**One change at a time.** Resist the urge to "fix everything." Each commit should be reviewable in isolation.

**No speculative abstraction.** Don't add flexibility for hypothetical future needs. Solve today's problem.

## Output

- Refactored code, committed incrementally
- Updated `.design/<branch>.md` if the approach warrants documentation
- Notes in `.design/questions.md` for deferred decisions
