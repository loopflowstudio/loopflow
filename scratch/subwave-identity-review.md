# Review-design notes: subwave identity (LOO-329)

2026-09-28, interactive review with Jack Heart. In progress.
Design: [define-durable-subwave-identity-and.md](define-durable-subwave-identity-and.md).
Kickoff draft under review: commit `0381a195f`.

## Feedback from Jack Heart

- "Where does Wave id come from? I dont think we should support wave ids.
  wave id = wave name. no suriving renames."
- Subwaves: "in most ways they should be treated as separate but its
  defintiely a deep data model change somehow."
- "tasks are part of projects, and projects can be moved to different
  wave-name. so task.wave = task.project.wave maybe? to not denormalize it too
  bad"
- The Wave file could be `wave/<wavename>/<WAVENAME>.md` with some structure;
  this space was tried before. "That said GOAL.md is also pretty simple." Not
  decided.
- "are we currently including wave/*? Should we be?" Raised as one of the
  biggest questions. Not decided.

- "ok so we include parent memory and goals but not children or sibling works
  i think". Recorded as the composition rule; it answers the question above.

- "so just wave/mywave/* and everything of your parents (not your siblings)".
  Read as top-level files of each directory on the path, never recursive.

- "scratch should be just recurssive, wave should be top level only, but all
  top-level .md (inlcudes memory)".

- "the 'name' of the release wave is infrastructure/release. so the parentage
  is implicit in the 'name'. we shouldnt need to track wave ids at all".
  Confirms dropping `parent_wave_id`.

- On a reused name showing both eras as one Wave: "Yeah i think thats correct
  behavior".

- On the checkout: "everything is in a worktree basically. there is no 'one'
  wave memory, its whatever is in a particular checkout for this task."

- "we should avoid product UXes that require using main's".

- On memory framing: "a little bit of hinting at hey this is a good canonical
  way to do things. but as much as possible being a regular citizen is great".

- "making release a real wave is like how we prototype the child wave setup".
  Brings the release split into scope.

- On Project moves: "projects themselves are relatively ephemeral. but often
  easiest way to transfer is to keep it alive. i was talking about how we might
  just need to update a field on the project that says wave name if we update
  the wave's name".

- On deriving a Task's Wave through its Project: "i dont think thats
  ultimately really the right tradeoff, since name changes are rare". Tasks
  record the Wave name.

- "im not sure on the right API. rebase on to 298." Done; effects recorded in
  the design's Base section.

- On Run history at rename: "for now, dont change history. maybe we add
  migrations later."

- "Maybe I'm wrong about no Wave UUID ... I think maybe we should keep them".
- "I still think parentage needs to be implicit from the name", then
  withdrawn: "actually no thats wrong to. release shoudlnt need to switch to
  infra/release if infrastructure changes to infra".
- "Ok, i guess we need raw parent id, with fast input / syntax or something".

- On a second machine: "this is at least allowing you to sync when there arent
  conflicts. Handling nasty conflicts can come a bit later".
- On `lf wave relocate`: "just fix it after 298 is done? idk".
- On cron Runs: "idk, research existing work exampels". Researched; see the
  design's Cron today section.

- On the app's Wave page: "dont chagne the desktop UI for noe".
- On release's objective: "just read whatever we have in exsiting copy for
  infra + release".
- On LOO-298's scratch: "probably mostly remove".

- On the drafted release Wave file: "this all seems mmaybe too formal or
  trying too hard. The objective of release is to get new, updated loopflow
  delivered to the public on regular intervals, with clear meaning, without
  bugs, in a recoverable and smart rollout strategy etc".

- "Let's dream to do a great job but not make it bureaucratic". The design
  was cut from 24 KB to one page; the evidence stays in earlier commits.

- On the objective: "with accessible guidance instead of clear meaning
  maybe?" and "a light editing touch would be appreciated".

- On parent and directory: "No. It can only live in infrastructure if
  parent_id is infra's".

- On a Task's Wave: "is wave stored as a name or and id? I dont relaly know
  or care but worth doing right whatever that means". Stored by id; left as
  LOO-298 has it.

## Design changes made

The table below records the first half of the review. The second half
withdrew name-as-identity: ids are kept, a Wave stores its parent's id, and
its name is one segment. The design document holds the current model.


| Kickoff | Now |
|---|---|
| Wave identity is a UUID; name is a mutable address | Name is the identity |
| Rename preserves identity through `wave/moves.jsonl` | No identity-preserving rename; ledger removed |
| Held scope: memory-only directory, no registry row | Removed; a subwave is a Wave |
| `tasks.scope` placement | Removed; `tasks.wave` holds the Wave name, validated against the Project |
| Promotion and demotion semantics | Removed with held scopes |
| Cron `scope:` field | Removed; a subwave owns its crons |

Kept from kickoff: ancestry computed from the path, `parent_wave_id` dropped.
Jack confirmed it.

## Evidence gathered in the session

- Wave ids are random UUIDs minted per Home by `WaveId::new()`
  (`controller/wave/registry.rs:43-64`). A remote Home adopts the origin's id
  through `ensure_wave_row_with_id`.
- `wave_id` appears 570 times in Rust source and in 16 Swift files.
- `tasks` already has no Wave column; `projects.wave_id` is the link.
- A prompt includes the bound Wave's top-level `*.md` and its `MEMORY.md`,
  nothing from other Waves or subdirectories.
- `wave/` is about 41,000 tokens in nine files; Infrastructure memory is about
  18,400 (tiktoken `o200k_base`, an approximation for Claude).

## Unresolved

See [questions.md](subwave-questions.md).

## Next useful action

Jack's answers on the open questions, then correct LOO-330 and LOO-331, whose
drafts still use held scopes and address history.
