# UX iteration loop

**Finish line:** A standing generator/discriminator loop for Concerto UX —
generate candidate variants, score them against explicit criteria, converge — as
a repeatable mechanism, not a one-off pass.

## Context

`wave-surface-ux-exploration` runs this by hand, once. This item makes it a
standing capability so the UX keeps improving without a bespoke effort each time.

- **Generator** — produce N UX candidates for a given screen or interaction.
- **Discriminator** — score them against explicit quality criteria (the metrics
  in desktop's GOAL).
- **Converge + record** — pick, ship, write the learning to MEMORY.

## Done when

- The loop can be pointed at a Concerto surface and return ranked candidates.
- At least one screen has been improved through the loop end-to-end.
- The mechanism is documented so it's reusable, not a one-off.
