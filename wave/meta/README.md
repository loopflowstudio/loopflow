# Meta

Make loopflow's own agent runs sharp. Meta owns what the model *sees and does*
— prompt text (gate, compress, implement), context assembly, and the local
record of every run — so each iteration of loopflow-on-loopflow gets cheaper
and more reliable.

Paired with **systems**: Systems keeps the machinery around the code fast;
Meta keeps the agent runs themselves effective. The lab is this repo,
cross-checked against cadenza: lessons that hold in both are general and move
into the builtins; lessons that hold in one are taste and stay in that repo's
agent file.

## North-star metrics

| Metric | Winning looks like |
|--------|--------------------|
| **Reconstructable runs** | Any run from the last N days can be replayed on paper: prompt, context, children, tokens, duration |
| **Measured prompt edits** | Gate/compress/implement changes cite observed runs; gate first-pass rate rises |
| **Tokens per run** | Trending down while quality holds; nothing large loads unread |
| **Paved-road adherence** | Worktrees, commits, landings go through `lf op`; deviations counted, each one fixed at the prompt or tool |
| **Flow legibility** | Every step/flow shows what it runs, how long, how hot; redundant ones die |
| **General/taste split** | Builtins carry what's universal; agent files carry the rest |

## The hard rule

Telemetry is local-only. No global server, no phone-home — run data never
leaves the user's machine. All logging and analysis tools operate on the
user's own data, for the user's own debugging.
