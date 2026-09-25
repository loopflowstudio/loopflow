# Current planning preview — 2026-09-25

Header feedback: the Task title now occupies the compact top content row and the
issue identifier links to its exact Linear issue. Removed the generic Open label,
oversized duplicate title, separate Open in Linear link, and the inline Session
attribution diagnostic. Unknown attribution still supplies no invented counts.
The exact-content/repository proof passes after this edit; a laptop browser check
verifies the identifier link and removal of visible diagnostic copy. The fresh
`task-header.png` capture was visually inspected. The earlier receipt's source
hashes describe the previous header, not this refinement.

The human requested exact current Linear data for Loopflow, Etude and Kata.
Refreshed the configured shared reads through `lf pm show --wave <name> --sync
--json` in each repository. Wave inventory came from `lf ls --all --current`.
No provider planning write, chapter rotation, installation or Session action ran.
Read refreshes update Loopflow's local PM cache.

The preview defaults to the captured current data. Loopflow exposes six Tasks;
Etude exposes six, including the provider-authored superseded ETU-78 record.
Task IDs, names and complete descriptions match the read exactly; Markdown is
rendered without paraphrasing. Wave objectives come from the shared Wave read;
chapter KRs are Linear-backed. These sources are distinct and neither is rewritten.
The current provider text supersedes the study's previously approved sample text.

Kata's seven Waves lack current chapter bindings. Its initial core/dj refreshes
also hit duplicate KAT-28 snapshot attribution. After the other snapshots refreshed,
a second bounded read of core/dj succeeded, but still reported missing chapters.
The preview preserves that unavailable state, not zero Tasks or synthetic plans.
Etude's study Wave lacks a configured Linear Initiative and is also unavailable.
No attempt was made to repair planning associations or manufacture current chapters.

The captured Session read has no typed Task Work attribution on any record.
Conversation counts cannot be inferred from titles or checkout paths. Current
planning mode therefore has an explicit attribution gap, no sample conversations,
transcripts, activity, retained-workspace claim or mutation controls. This is a
planning/content review, not a native conversation or provider demo.

Sidebar membership uses positive shared local-progress evidence or an exact Task
selector on recorded execution. Merely having a runtime/worktree record is not
treated as started work. This limited snapshot may omit previously started Tasks
whose history is outside the recorded read; native implementation still needs
the complete shared started-work contract.

`build.py` renders the captured source into `current-data.js`; `current-plan.js`
loads it into the study's existing navigation owner. `check.py` passes exact
comparisons for all 12 Task names/descriptions and 14 Wave objective/KR values,
switching among all three repositories, full Wave plan inspection, unavailable
versus empty presentation, and absence of synthetic Session/mutation controls.
The first test used an overly broad h1 selector: the actual Linear directive
contains its own headings. Corrected it to target the page title; no product
behavior was altered to pass that assertion. Captures use `lf screenshot`.

Source reads occurred during this turn, not atomically across repositories. This
is a dated snapshot, not a promise of continuous freshness. No claim is made that
Kata's Linear Tasks are fully represented while its chapter projection is unavailable.
