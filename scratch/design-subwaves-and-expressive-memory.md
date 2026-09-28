# Subwaves and memory addresses (Product · LOO-330)

Status: **draft for review-design with Jack Heart.** Nothing here is accepted.
No implementation, registry write, or live Wave ownership change follows from
this document. Companion designs: Infrastructure
[LOO-329](https://linear.app/loopflow/issue/LOO-329) (durable identity and
preservation) and Intelligence
[LOO-331](https://linear.app/loopflow/issue/LOO-331) (automatic inclusion and
measured allocation).

## Problem

Jack Heart selected `wave/infrastructure/release/MEMORY.md` as the home for
release findings. The file exists and nothing reads it. Three things are
missing, and each one hurts a different reader:

- **A person** cannot say where a finding lives without pasting a file path, and
  cannot tell from any surface that release memory exists, who reads it, or
  whether it is part of Infrastructure's prompt.
- **An agent** working on release receives all 1,065 lines of Infrastructure
  memory and none of the release file. An agent curating memory has one choice
  of destination per Wave, so every Wave memory grows without a place to put
  narrower facts.
- **Every Run** pays for the whole Wave memory. Sizes on 2026-09-28
  (characters ÷ 4, an estimate, not a tokenizer count):

  | Scope | Lines | ≈ Tokens |
  |---|---|---|
  | `infrastructure` | 1,065 | 21,700 |
  | `product` | 803 | 14,200 |
  | `intelligence` | 321 | 5,300 |
  | `growth` | 217 | 2,900 |
  | `infrastructure/release` | 76 | 1,300 |

The request is a product model for nested Waves and one addressing system that
people and agents can both say out loud, with release as the worked example.

## The demo

Jack runs `lf memory ls` in the loopflow repository and sees the tree with
`infrastructure/release` under `infrastructure`, marked as held by
Infrastructure. He runs `lf memory show infrastructure/release --as-read` and
sees exactly what a release Run receives: Infrastructure's memory labeled
inherited, release's labeled owned, and a one-line index of anything not
included. He then starts a Task placed at `infrastructure/release`; its prompt
carries the same two blocks with the same labels, and the Task page shows the
same composition behind one chip.

## Approach

### Three concepts, one address

| Concept | What it is | How you recognize it |
|---|---|---|
| **Scope** | A place memory lives | A directory under `wave/` containing `MEMORY.md` or `GOAL.md` |
| **Subwave** | A scope nested under another scope | Its address has more than one segment |
| **Operated** | A property: the scope has its own objective, chapter plan, cadence, chat, and budget | It has `GOAL.md` |

Every Wave is a scope. A subwave without `GOAL.md` is **held**: its nearest
operated ancestor operates it. It owns memory and groups Tasks, and borrows
everything else. `infrastructure/release` today is a held subwave of
`infrastructure`.

Nested directories that contain neither file are not scopes. `metrics/` under a
Wave stays a metric-contract folder.

Adding `GOAL.md` later makes a held subwave operated. Removing it makes it held
again. **Neither changes its address.** This is the answer to "does every
nested directory become an independently operated Wave": no, and the choice is
reversible without breaking a single reference.

### The address

```
infrastructure/release                       a scope
infrastructure/release#observed-chain        one entry in its memory
loopflow:infrastructure/release              the same scope, named from another repository
```

- A scope address is its path below `wave/`. The memory file is always
  `wave/<address>/MEMORY.md`. An agent with no tool can read any address.
- An entry is a heading in that file, addressed by GitHub's heading slug. The
  three cross-memory links that exist today already use this form.
- The repository qualifier appears only when the reader is outside the
  repository: Linear, another repository's memory, cross-repo chat.

The same address selects the subwave everywhere a Wave name is accepted today:
`lf status infrastructure/release`, `-w infrastructure/release`, the
`wave:infrastructure/release` Run subject. `WaveLocator` already accepts
slash-qualified slugs, and `lf wave relocate` already carries path-nested
descendants.

### Relative and fully qualified

| Form | Example | Where it is allowed |
|---|---|---|
| Fully qualified | `infrastructure/release` | Everywhere. The only form Loopflow prints or persists. |
| Relative | `./release`, `..`, `../intelligence` | Typed input, resolved against the ambient scope; Markdown links inside memory files |
| Leaf shorthand | `release` | Typed input only, when exactly one scope ends in that segment |

Rules:

1. An address without a leading `.` is read from the root first. `release`
   means root-level `release` when that exists.
2. Otherwise a bare leaf resolves when exactly one scope ends with it, and the
   command echoes what it chose: `release → infrastructure/release`.
3. When two scopes match, nothing is chosen. The command lists the candidates
   and exits nonzero:

   ```
   $ lf memory show release
   "release" names two scopes:
     infrastructure/release
     growth/release
   ```

4. Loopflow never stores a relative or shorthand address. Tasks, PR bodies,
   Linear comments, Run subjects, and receipts carry the fully qualified form,
   so a later `growth/release` cannot change what an old record meant.
5. Inside memory files, cross-references are ordinary relative Markdown links
   (`[release memory](release/MEMORY.md#observed-chain)`). They render on
   GitHub, and links inside a subtree survive moving that subtree.

### Inheritance and union

Two directions, different defaults.

**Down (inheritance), automatic.** A Run placed at a scope reads every ancestor
memory, root first, then its own. Whole files, in path order.

**Up (union), on request.** A Run placed at a parent does not receive child
memory. It receives an index: each child's address, first sentence, and size.
A reader who needs the content asks for it by address.

**Sideways, never.** Siblings are not included and not indexed beyond what the
parent's index shows.

Worked example, assuming a second held subwave `infrastructure/auth` exists:

| Run placed at | Reads in full | Sees in index | Never sees |
|---|---|---|---|
| `infrastructure` | `infrastructure` | `release`, `auth` | — |
| `infrastructure/release` | `infrastructure`, then `infrastructure/release` | children of release | `auth`, `product` |
| `infrastructure/auth` | `infrastructure`, then `infrastructure/auth` | children of auth | `release` |

Union is what curation and review need, so it is explicit:
`lf memory show infrastructure --union` prints the parent and every descendant,
each labeled. Chapter review and `update-wave` use it; ordinary Task Runs do
not.

Why not union by default: full union means moving a section into a subwave
saves nothing for the parent and the parent's prompt grows with every child.
Sibling isolation is the only saving nesting offers, and union by default
spends it. LOO-331 owns measuring whether the index is enough; the default here
is the Product proposal going into that review.

Consequence worth stating plainly: inheritance makes a release Run *more*
expensive than an Infrastructure Run today (≈21,700 + 1,300). The saving arrives
only as release-specific sections leave the parent. Nesting is a place to move
things to, not a discount by itself.

### Ownership and inclusion cues

The prompt states what was included, from where, and why. It replaces today's
`## Memory inherited from <name>` headers:

```
<lf:wave-memory scope="infrastructure/release">
<lf:memory address="infrastructure" role="inherited" path="wave/infrastructure/MEMORY.md">
…
</lf:memory>
<lf:memory address="infrastructure/release" role="owned" path="wave/infrastructure/release/MEMORY.md">
…
</lf:memory>
<lf:memory-index>
Not included. Read by address when needed.
- infrastructure/auth — Account auth, browser profiles, and OAuth recovery. (≈3,100 tokens)
</lf:memory-index>
</lf:wave-memory>
```

Three roles, used identically in the prompt, CLI, and app:

| Role | Meaning | Write here? |
|---|---|---|
| **owned** | The scope this work is placed at | Yes, by default |
| **inherited** | An ancestor, included because this scope sits below it | Only for facts every descendant needs |
| **indexed** | A child, listed and not included | Only when curating that child |

### Choosing where a memory belongs

One rule, taught in `update-wave` and `record-learnings`:

> Write a finding at the narrowest scope whose every reader needs it.

Applied to release:

| Finding | Belongs at | Why |
|---|---|---|
| "Cron must observe the operation's result" | `infrastructure/release` | Only release work acts on it |
| "An entry point must preserve recovery ownership" | `infrastructure/release`, one-line pointer in `infrastructure` | Found in release; the principle applies to every landing path |
| "Preserve the populated Home before proposing a new chapter" | `infrastructure` | Every Infrastructure subwave can trip on it |
| "PR openings explain the user's experienced change" | `product` | Not Infrastructure's at all; a link, not a copy |

A finding that two siblings need moves up one level. A finding that only one
child reads moves down. A finding another Wave owns is linked, never copied.

### Read and write flows

**Discover.**

```
$ lf memory ls
product                      ≈14,200   operated
infrastructure               ≈21,700   operated
  release                     ≈1,300   held by infrastructure
intelligence                  ≈5,300   operated
growth                        ≈2,900   operated
```

**Read one scope.** `lf memory show infrastructure/release` prints the file.
`#observed-chain` prints one entry. `--json` returns address, path, role, size,
and content.

**Read as a Run would.** `lf memory show infrastructure/release --as-read`
prints the composed blocks above. This is the single answer to "what does my
agent know", and it is the same code path prompt assembly uses.

**Write.** Edit `wave/<address>/MEMORY.md` and commit. There is no write
command. The file stays the whole memory, reviewed like code.

**Create a held subwave.** Make the directory and write `MEMORY.md` whose first
sentence says what the scope is for. That sentence is the index line. No
registration step.

**Operate a subwave.** Add `GOAL.md` and register through the existing Wave
path. A person decides this; an agent may propose it.

**Place a Task.** A Task belongs to its operated Wave's chapter and carries a
scope at or below that Wave: `lf task create --wave infrastructure/release …`
files the Task in Infrastructure's chapter, placed at release. Unplaced Tasks
read the Wave's own scope, which is today's behavior.

### Rename and move

An address is a locator. Identity is LOO-329's. The experience Product requires:

1. `lf wave relocate infrastructure/release --name delivery/release` moves the
   directory and its descendants in one operation, as it does for registered
   Waves today.
2. The same operation rewrites inbound Markdown links and qualified addresses
   in tracked memory and docs, in the same commit, and prints what it changed.
3. An old address still resolves, visibly:

   ```
   $ lf memory show infrastructure/release
   infrastructure/release moved to delivery/release on 2026-10-02.
   ```

   followed by the content. Records that captured the old address stay
   readable without being rewritten.
4. A reused name never silently answers for the old scope. If a new
   `infrastructure/release` is created later, it wins for new reads and says
   that an earlier scope held the address until the move date.
5. Renaming a heading is a move of an entry. `lf memory check` reports inbound
   links the rename broke.

### App presentation

The accepted hierarchy stays Repo → Wave → Task → Session.

- **Sidebar.** Operated subwaves appear as indented Wave rows under their
  parent, same row component, sans, no glyph. Held subwaves do not appear in
  the sidebar: they are not something you operate.
- **Breadcrumb.** `loopflow › infrastructure › release › LOO-328`. Each segment
  is a link; held segments open the parent Wave page scrolled to that subwave.
- **Wave page.** A Memory section lists the scope tree below this Wave: address,
  first sentence, size, and `held` or `operated`. The Task plan groups Tasks by
  placement, with the Wave's own Tasks first.
- **Task page.** One quiet mono chip beside the worktree chip:
  `infrastructure/release`. Opening it shows the composition as three labeled
  rows (inherited, owned, indexed) with sizes. Nothing else is added to the
  page.
- **Session view.** Unchanged. The breadcrumb carries the scope.

### CLI summary

| Command | What it does |
|---|---|
| `lf memory ls [ADDRESS]` | Prints the scope tree with sizes and held/operated |
| `lf memory show ADDRESS[#entry]` | Prints one scope or entry; `--as-read` composes what a Run receives; `--union` includes descendants |
| `lf memory check` | Reports broken links, scopes without a first sentence, and addresses that resolve through a move |

All three read files. None writes, caches, or registers.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Does anything read `infrastructure/release/MEMORY.md` today? | No. `Memory::for_wave` reads `wave/<wave>/MEMORY.md` only, and `gather_wave_docs` reads immediate Markdown in the Wave directory while excluding `MEMORY.md`. The file is reachable only through the link at line 5 of Infrastructure memory. | The release file is currently invisible to every prompt. The design must make inclusion explicit, since a silent gap already exists. |
| Can an address contain `/`? | Yes. `WaveLocator::new` accepts slash-separated slugs and rejects empty, `.`, and `..` components. `normalize_wave_name` strips a leading `wave/`. | Path-shaped addresses need no new grammar. |
| Is nested discovery already defined? | `collect_local_waves` recurses under `wave/` and treats a directory as a Wave when it contains `GOAL.md`. | "Operated means `GOAL.md`" is the existing rule, not a new one. Held scopes are what that rule already skips. |
| Does inheritance exist? | Yes, through the registry. `memory_wave_chain_from_store` follows `parent_wave_id` and renders ancestors first with `inherited from` / `owned by` headers. The test pairs `platform` and `release` as path siblings joined only by a registry link. | Two hierarchies exist: registry parent and path. They can disagree. The product model shows one. |
| Are any live Waves nested today? | `lf ls --json` on 2026-09-28 shows `parent_wave_id: null` for all six registered Waves. | No live ancestry has to be preserved or migrated to adopt path order. |
| Does relocation carry children? | `plan_moves` moves descendants whose slug is path-nested under the renamed root. Docs state the same. | Rename of a subtree is existing behavior for registered Waves. Held scopes and inbound link rewriting are the gap. |
| Do cross-memory links exist? | Three, all relative Markdown with GitHub heading slugs, two of them with dates in the slug. | Entry addresses by heading slug match current practice. Dated headings make long slugs; see open tradeoffs. |
| Was a memory CLI deliberately refused? | Yes. Intelligence memory and `docs/agent-api.md` say the file is the whole surface and there is no CLI for it. | `lf memory` must stay a reader. A write verb or a cache would reverse an accepted decision. |
| What do LOO-329 and LOO-331 say? | Both workers were running kickoff with empty `scratch/` when this draft was written. Jack's comments on LOO-331 ask for depth, ordering, deduplication, sibling boundaries, and precedence. | Alignment below is this design's proposal, not an agreed contract. It needs reconciling at review. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Every nested directory is an operated Wave | One concept. Each area needs a GOAL, Linear Initiative, chapter plan, cadence, and chat before it can hold a note. | Release needs a place for findings, not a second operator. The cost stops people from creating scopes. |
| Flat Waves, topic files inside one Wave (`infrastructure/release.md`) | No nesting. Simple paths. | No Task placement, no inheritance order, no path to operating the area later without changing its address. |
| Tags on entries, scopes computed by query | Flexible; one entry can belong to several areas. | Needs a parser and an index, and the answer to "what does my agent read" stops being a list of files. |
| Opaque stable IDs as addresses (`scope_8f3a…`) | Survives every rename. | Nobody can say it, type it, or guess it. Identity belongs beside the address, not in place of it. |
| Registry parent as the hierarchy, names unrelated to paths | Already implemented for chains. | People see paths in the repository and on GitHub. A hidden second tree is the ambiguity this work exists to remove. |
| Union by default | Parent agents see everything. | Removes the only saving nesting provides and grows parent prompts with every child. |

## Key decisions

1. **Path is the hierarchy people see.** One tree, readable in the repository,
   the CLI, the app, and GitHub.
2. **Operated is a property, not a kind.** Held and operated subwaves share one
   address form, so promotion never breaks a reference.
3. **Loopflow prints and stores only fully qualified addresses.** Shorthand is
   an input courtesy.
4. **Ambiguity resolves to nothing.** Candidates are listed; no guess is made.
5. **Inheritance is automatic; union is requested; siblings are excluded.**
6. **Children are indexed to the parent.** The parent knows what exists and
   what it costs without reading it.
7. **The file remains the memory.** `lf memory` reads, composes, and checks.
8. **Memory placement rule is one sentence**, taught where curation happens.

## Boundaries with Infrastructure and Intelligence

| Question | Owner | What Product needs from the answer |
|---|---|---|
| What is a held scope's durable identity, and where does it live? | LOO-329 | An old address resolves with a visible moved notice; a reused name never answers for the old scope. |
| Does `parent_wave_id` derive from the path, or must it agree with it? | LOO-329 | One hierarchy on every surface. |
| Where is a Task's placement stored, and how does Linear show it? | LOO-329 | Placement survives chapter rotation and is readable from `lf task status --json`. |
| Are move notices files, registry rows, or Git history? | LOO-329 | `lf memory show <old>` works with every server stopped. |
| Is the child index enough for parent Runs? What does each skill need? | LOO-331 | Evidence from real prompts, per skill. |
| Depth limit, ordering, deduplication, precedence when ancestor and child disagree | LOO-331 | One rule a person can predict. Product's proposal: root first, whole files, nearest scope wins on conflict, no deduplication. |
| Size budgets per scope, and what happens past them | LOO-331 | A number shown in `lf memory ls`, and a warning, never a truncation. |
| Does the Run record capture which addresses were included, with roles? | LOO-331 | The Task page chip reads recorded composition, not a recomputation. |

Shared vocabulary proposed to all three: **scope**, **subwave**, **operated**,
**held**, **address**, **entry**, **owned**, **inherited**, **indexed**,
**union**.

## Unresolved tradeoffs

Held open for Jack's review.

1. **Who may create a held subwave.** Agents creating scopes during curation
   keeps memory tidy and risks a sprawl of tiny scopes. People-only creation
   keeps the tree deliberate and leaves big memories big. This draft lets
   agents create and requires the PR to say so.
2. **Dated headings as entry addresses.** Current headings carry dates and
   evidence qualifiers, so slugs are long and change when curation retitles a
   section. Short stable headings with the date in the body read better as
   addresses and lose the at-a-glance date in the outline.
3. **Whole-file inheritance.** Simple and predictable, and a release Run still
   pays for all of Infrastructure. Entry-level inheritance would cut cost and
   needs a marking scheme nobody has asked for.
4. **Held subwaves in the sidebar.** Hidden keeps the sidebar to things you
   operate. Shown would make release Tasks findable under their area.
5. **Leaf shorthand at all.** It is convenient today with five scopes and
   becomes a source of "which release" as soon as a second one exists.
6. **The word "subwave".** A held subwave operates nothing, so "wave" may
   overpromise. "Area" is the word release memory already uses.

## Scope

- In scope: the concept model, address grammar, resolution rules, inheritance
  and union defaults, prompt cues, `lf memory` reads, Task placement
  experience, move experience, and app presentation.
- Out of scope: identity storage and move records (LOO-329); measured
  allocation, budgets, and per-skill inclusion (LOO-331); entry-level
  inheritance; tags; a memory write command; cross-Home memory; splitting any
  existing memory file; registering, promoting, or relocating any live Wave.

## Done when

For this Task: Jack Heart completes review-design on this draft together with
the LOO-329 and LOO-331 drafts, and the three agree on one vocabulary and one
composition rule.

For the eventual build:

- `lf memory ls` in the loopflow repository lists `infrastructure/release` as
  held by `infrastructure`.
- `lf memory show infrastructure/release --as-read` and the recorded prompt of
  a Task placed there contain the same two blocks with the same roles.
- A Task placed at `infrastructure` receives release as an index line and not
  as content.
- `lf memory show release` succeeds with an echo while one scope matches and
  lists candidates when two do.
- After a relocation, the old address resolves with a moved notice and
  `lf memory check` reports no broken links.

## Forbidden outcomes

- A memory store, cache, index file, or database table that holds memory
  content or can disagree with the files.
- A registry parent link that differs from the path and still shapes what a
  Run reads.
- A stored relative or shorthand address.
- An ambiguous address that resolves to the first, newest, or nearest match.
- Child memory included in a parent prompt without a label saying so.
- A held subwave that acquires cadence, chat, or a chapter plan without
  `GOAL.md`.
- An old and a new composition path living side by side; the registry-chain
  reader is replaced, not wrapped.
- Truncating a memory to fit a budget.

## Internal slices

1. **Resolve and list.** Address parsing, resolution rules, `lf memory ls` and
   `show`. Read-only, no prompt change.
2. **Compose.** Path-ordered inheritance, child index, role-labeled blocks,
   `--as-read` and `--union`. Replaces the registry-chain reader. Prompt
   fixtures change once.
3. **Place.** Task placement at a scope, shown in `lf task status --json`.
   Depends on LOO-329.
4. **Teach.** The placement rule in `update-wave` and `record-learnings`; docs
   that say "no CLI surface" updated to say "no write surface".
5. **Move.** Held-scope relocation, inbound link rewriting, moved notices,
   `lf memory check`. Depends on LOO-329.
6. **Show.** Wave page Memory section, Task page chip, breadcrumb segments,
   operated subwave rows.

## This slice

This draft and its review. No code.

## Slice ledger

- 2026-09-28: Draft written from source reading and `lf ls --json`. Token
  figures are character estimates. LOO-329 and LOO-331 had produced no design
  text; alignment is proposed, not agreed.

## Measure

Before: memory characters and tokenizer count per scope, and memory tokens in
the recorded prompt of one Infrastructure Task and one release-focused Task.
After slice 2: the same two prompts. Better means the release Task reads
release memory it did not read before, and the Infrastructure Task's memory
does not grow. After sections move into subwaves: a Task placed at a sibling
reads fewer memory tokens than an Infrastructure Task does today. LOO-331 owns
the instrument.
