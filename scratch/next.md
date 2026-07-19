# Slice 4: remove ambient Wave chat from prompts

## Implement

- Delete `PromptComponents.wave_chat`, its gather/render/budget/debug/trace
  paths, and the `<lf:wave-chat-recent>` prompt section.
- Delete live/journal Wave-chat fallback from generic Project/Task prompt
  assembly. Recent transcript order is never an implicit context rule.
- Narrow skill launch seeding so Wave context is deliberate: applicable
  `MEMORY.md` may be selected, arbitrary recent chat may not.
- Update builtin wording, docs, Swift comments, tests, traces, and goldens.

## Preserve

- Wave conversation, `ChatTurn`, listener journal, `lf chat`, and the Mac Wave
  chat UI as product surfaces pending server-topology design.
- Explicitly selected child Turn/output used to conduct Feedback.
- Ancestor `MEMORY.md` prompt context.

## Done when

- [ ] `PromptComponents` and prompt assembly have no Wave chat field/tag.
- [ ] Project/Task prompt tests seed real recent Wave turns and prove they are
      absent while selected memory remains.
- [ ] No general runtime or prompt path calls `gather_wave_chat`.
- [ ] Focused prompt/context/run/builtin/golden proofs, fmt, and clippy pass.
