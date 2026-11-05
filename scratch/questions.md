# Open questions for review (c0b373)

Assumptions made in the headless implement step, each reversible.

1. **Interactive continuation.** The login moves by itself; the interrupted turn continues on the person's next message. Nothing types into a terminal, to keep active input safe. Automatic continuation of idle Sessions would need a reliable "no draft in progress" signal.
2. **No model or harness fallback.** Preferences hold one resolved agent with no ordered alternates, so any fallback would invent policy. A model-scoped limit cools the whole account.
3. **A rejected event cools the account until its reset**, including `seven_day_overage_included`. If overage rejection leaves the plan's own quota usable, this cools too much.
4. **Waits have no upper bound.** A weekly limit can hold a step for days, in one idle process, until someone interrupts it.
5. **Isolated Session resume** no longer passes its recorded account as exact, so a cooled account moves the conversation instead of refusing. The pin still wins whenever that account is eligible.
