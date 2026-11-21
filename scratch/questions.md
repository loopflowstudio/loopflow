# Questions

Choices made without a person, open to reversal. `scratch/concept-review.html`
shows them.

1. A relocated flag missing its value makes `reorder_args` return the shortest
   argv that reaches it (`lf --repo`, `lf task create --wave`), so Clap prints
   its missing-value error. Passed over: keeping everything written before the
   flag, leaving the flag in place (`unexpected argument`; for a skill, the
   flag becomes a prompt word), and making root flags Clap globals.
2. `lf : fix the --repo flag` sets the repository to `flag` today. A trailing
   `--repo` there becomes a missing-value error. Should prose after `skill:`
   keep root-flag words literally? Not changed here.
3. `lf.rs` and `navigation.rs` each keep a reading of "does this flag take a
   value". Merging the scanners is left as its own Task.
