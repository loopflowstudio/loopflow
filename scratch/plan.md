# Installed skills reach every harness

## Outcome

After `lf self sync-skills --yes` (which `lf install` already runs), Antigravity's
`/skills` lists the same Loopflow skills and Graphs as Claude and Codex. opencode
and pi already see them and keep doing so. Each harness says where it reads
installed skills, the export loop writes exactly those directories, and a sixth
harness that says nothing fails a test.

## What the probes showed (2026-11-21, this Machine)

| Harness | Version | Global | Repository | Source |
|---|---|---|---|---|
| claude | — | `~/.claude/skills` | `.claude/skills` | existing export |
| codex | — | `~/.agents/skills` | `.agents/skills` | existing export |
| antigravity | agy 1.5.14 | `~/.gemini/config/skills` only | `.agents/skills`, cwd up to the repository root | bundled docs `~/.gemini/antigravity-cli/builtin/skills/agy-customizations/`, plus runs below |
| opencode | 2.0.32 | `~/.claude/skills` and `~/.agents/skills` | `.claude/skills`, `.agents/skills`, `.opencode`, walking up | strings of the binary: `claude:…join(home,".claude")`, `agents:…join(home,".agents")`, each scanned at `skills/{*.md,**/SKILL.md}`; later directory wins a name, so `.agents` wins |
| pi | 1.3.0 | `~/.agents/skills` (and `~/.pi/agent/skills`) | `.agents/skills`, cwd up to the repository root | `docs/skills.md` in the installed package |

So the only gap is Antigravity's global directory. opencode and pi need no new tree.

Antigravity runs, each `cd /tmp/agyprobe && agy -p "/skills" < /dev/null` (prints
`name<TAB>description` per skill, no model call, about 10 s):

- Baseline: 7 builtins.
- `~/.gemini/config/skills/lf-probe-tree/SKILL.md` with frontmatter `name`,
  `description`, `loopflow: true`, `loopflow-kind: graph`,
  `disable-model-invocation: true`: listed. Extra frontmatter keys are accepted.
- `~/.gemini/config/skills.json` = `{"entries":[{"path":"~/.agents/skills"}]}`:
  **nothing added**. The docs say `~/` resolves; 1.5.14 does not.
- Same file with the absolute path `/Users/…/.agents/skills`: 116 listed (7 + all 109
  folders there).
- `~/.gemini/config/skills` as a symlink to a directory of skills: followed.

Both probe files were removed; `~/.gemini/config` is back to `config.json`,
`mcp_config.json`, `projects`.

## Decisions

**Antigravity gets a generated tree at `~/.gemini/config/skills/`** — made for
review. It is the same kind of thing as the two trees Loopflow writes today:
marked files (`loopflow: true`) that the existing write, collision and prune
code already owns, and it ranks above agy's builtins. Passed over: a
`skills.json` entry. It means merging into a provider-owned config file, it only
works with an absolute home path baked in (the `~/` form fails on 1.5.14), it
ranks below agy's builtins, and it is the kind of write into global provider
settings Loopflow doesn't make. What the tree gives up: skills other tools put
in `~/.agents/skills` (`find-skills`) stay invisible to agy.

**The declaration lives on `Capabilities`** (`rust/loopflow-execution/src/harness/mod.rs`),
beside `native_skills`:

```rust
pub struct InstalledSkills {
    pub global: &'static [SkillDirectory],      // relative to the home directory
    pub repository: &'static [SkillDirectory],  // relative to the repository root
}
pub struct SkillDirectory {
    pub path: &'static str,
    pub dialect: SkillDialect,
}
```

| Harness | global | repository |
|---|---|---|
| claude | `.claude/skills` Claude | `.claude/skills` Claude |
| codex | `.agents/skills` Codex | `.agents/skills` Codex |
| antigravity | `.gemini/config/skills` Codex | `.agents/skills` Codex |
| opencode | `.agents/skills` Codex | `.agents/skills` Codex |
| pi | `.agents/skills` Codex | `.agents/skills` Codex |

opencode declares only `.agents/skills`: it also reads `.claude/skills`, but
`.agents` wins on a shared name, so that is the copy it shows.

`SkillDialect` replaces the private `Vendor` enum. Its only use is
`render_graph_skill`, where the Claude dialect adds `disable-model-invocation: true`.

**No "has none, because…" variant** — made for review. All five harnesses read a
directory, the field is required so a new `Launcher` won't compile without it,
and the conformance test rejects an empty list. A reason variant arrives with the
first harness that needs one.

**`native_skills` stays `None` for antigravity, opencode and pi** — made for
review. It decides how a skill authored in another harness's dialect is invoked
(`/name` on Claude, `$name` on Codex); Loopflow-dialect skills never take that
path (`prompt/process.rs:170`). Turning it on needs an invocation syntax per
harness (`/name` in agy print mode, `/skill:name` in pi, nothing certain in
opencode) and a check that the source sits where that harness reads. Installed
skills make that possible; it is its own change.

**The tree is written whether or not Antigravity is installed**, as
`~/.claude/skills` and `~/.agents/skills` are today. No installed-check.

## Source of truth

`launcher(kind).capabilities().installed_skills` for each `HarnessKind::ALL`.
`sync_skills` folds those into a deduplicated, ordered list of
`(directory, dialect)` for the scope it was asked for (`global` without
`options.repo`, `repository` with it). One public function in `skills/mod.rs`
returns that list; `run_sync_skills` builds its confirmation and refusal text
from it.

## Consumers

- `sync_skills` (`rust/loopflow-execution/src/skills/mod.rs:78`): the loop over
  the hardcoded pair becomes a loop over the derived list. Write, block, prune
  and symlink rules are untouched.
- `run_sync_skills` (`rust/loopflow/src/lf/commands/ops/mod.rs:333`): both
  strings name the derived directories.
- `lf install` (`install.rs:2933`) calls `run_sync_skills(true, false, false)`;
  an upgrade writes the new tree with no further change.
- `docs/config.md:372-375`, `rust/loopflow/tests/README.md:512`,
  `scripts/benchmarks/pi/README.md:76` (says `lf sync-skills`; the command is
  `lf self sync-skills`): name the three global directories and which harness
  reads which.
- Skill discovery (`skills/catalog.rs:295`) keeps its own list of source
  directories. Generated files are skipped there by the `loopflow: true` marker,
  so the new tree can't feed back in.

## Absent and error states

- No `~/.gemini`: sync creates `~/.gemini/config/skills/`.
- A hand-written skill at `~/.gemini/config/skills/<name>`: kept and reported as
  skipped, by `writable_target`, as in the other trees.
- `~/.gemini/config/skills` is a symlink: nothing is written or pruned through
  it (existing rule in `writable_target` and `prune_targets`).
- Two harnesses declaring one path with different dialects: the conformance test
  fails; there is no runtime branch for it.
- agy drops a skill whose frontmatter `name` differs from its folder or lacks
  `description`. `render_skill` and `render_graph_skill` write `name` as the
  export folder name and always a description; the acceptance test asserts both.

## Acceptance check at gate

```bash
cargo test -p loopflow-execution --lib skills::tests
cargo test -p loopflow-execution --lib harness::conformance_tests::every_harness_reads_a_synced_skill
```

The second is new. It syncs into a temporary home (builtins: `implement`, Graph
`code`) and into a temporary repository seeded with one skill and one Graph
(a repository sync exports only the repository's own), then for every
`HarnessKind::ALL` and every directory it declares for that scope asserts: the
list is not empty; the skill's and the Graph's `<dir>/<name>/SKILL.md` exist;
frontmatter `name` equals the folder name; `description` is a non-empty string. It also asserts one dialect per path
across harnesses. Break the Antigravity declaration and it fails.

Not runnable in CI (needs the real binaries), for demo and qa:

```bash
cargo run -p loopflow --bin lf -- self sync-skills --yes
cd /tmp && agy -p "/skills" < /dev/null | cut -f1     # includes implement, kickoff, code
```

## Demo

`lf self sync-skills --yes`, then `/skills` in agy shows `implement`, `kickoff`,
`code` and the rest beside its seven builtins; `/skills reload` in a session
that was already open. The confirmation prompt without `--yes` names all three
directories.

## Exclusions

- Native skill invocation for antigravity, opencode and pi.
- Reading hand-written skills from `~/.gemini/config/skills`, `~/.pi/agent/skills`
  or `~/.config/opencode` as Loopflow skill sources.
- A `skills.json` entry, or carrying third-party `~/.agents/skills` bundles into agy.
- Checking opencode and pi by listing skills in a live session; the table above
  rests on the binary's strings and pi's bundled docs.

## What goes

- `enum Vendor` in `skills/mod.rs`.
- The literal pair in `sync_skills` and the two literal directory strings in
  `run_sync_skills`.

Survives: every existing export, marker, collision, prune and symlink test in
`skills::tests`; files already in `~/.claude/skills` and `~/.agents/skills` are
byte-identical after the change.

## Slices

1. **Declaration drives the export.** Add `InstalledSkills` to `Capabilities`
   with the table above; `sync_skills` derives its targets; delete `Vendor`.
   Write `every_harness_reads_a_synced_skill`. Then run the branch `lf` against
   the real home and `agy -p "/skills"`. This is the slice that could prove the
   plan wrong: it shows whether agy accepts all ~95 real exports (long
   descriptions, Graph recipes with input files beside `SKILL.md`), not just the
   probe skill. If some are dropped, the fix is in rendering, found here.
2. **Text from the same list.** `run_sync_skills` confirmation and refusal,
   `docs/config.md`, the two READMEs.
