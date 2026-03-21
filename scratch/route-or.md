path: tune

Both waves are shipping fast, but the dependency sequencing between them is misaligned. Chord-model is building depth-first on engine internals while agent-embedding is about to run out of unblocked items. Pulling chord-wave-area-model forward and resolving the lfd reachability issue would keep both waves productive. Without adjustment, agent-embedding stalls within one iteration and the tend cycle can't be validated live. These are chord-level sequencing decisions, not things the member waves will self-correct.
