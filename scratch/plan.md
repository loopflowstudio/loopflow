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
beside `native_skills`. It is one path per scope and nothing else:

```rust
pub struct InstalledSkills {
    pub global: &'static str,      // relative to the home directory
    pub repository: &'static str,  // relative to the repository root
}
```

| Harness | global | repository |
|---|---|---|
| claude | `.claude/skills` | `.claude/skills` |
| codex | `.agents/skills` | `.agents/skills` |
| antigravity | `.gemini/config/skills` | `.agents/skills` |
| opencode | `.agents/skills` | `.agents/skills` |
| pi | `.agents/skills` | `.agents/skills` |

It names where Loopflow puts skills for the harness, one directory per scope,
not every directory the harness reads (opencode also reads `.claude/skills`, pi
`~/.pi/agent/skills`).

**One rendering for every directory; a directory has no dialect** — made for
review. `.agents/skills` is read by codex, pi, opencode and, in a repository,
Antigravity, so no single harness's dialect describes it. The only thing the
private `Vendor` enum decides is whether a Graph recipe carries
`disable-model-invocation: true`, and it leaves the line out of `.agents/skills`.
pi honors that key (`docs/skills.md:53` in its package), so today pi may pick a
Graph recipe on its own while Claude may not. Every Graph export now carries the
line; `Vendor` and the `vendor` argument go, and nothing replaces them. agy
listed a skill carrying the key (probe above). Skill exports copied from a
Claude-authored source already carry it into `.agents/skills`
(`chat_recipes_preserve_native_user_only_controls`). Passed over: a
`SkillDialect` per directory, which would label `~/.gemini/config/skills` and
pi's directory "Codex" and need a test that two harnesses agree on a shared
path. If Codex turns out to reject or mis-handle the key, slice 1 finds it and
the answer is still one rendering, without the line.

**No "has none, because…" variant** — made for review. All five harnesses read a
directory and the field is required, so a new `Launcher` won't compile without
it, and a path can't be left empty the way a list could. A reason variant arrives with the
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
`sync_skills` folds those into a deduplicated, ordered list of directories for
the scope it was asked for (`global` without
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
- A harness moves its directory in a later version (agy 1.5.14's docs and
  behavior already disagree about `skills.json`): the declaration changes, the
  new tree is written, and the old tree stays where it was. Nothing records
  where an earlier `lf` wrote. The files are marked `loopflow: true` and are
  removed by hand; no retired-directory list is built for this.
- A Loopflow skill sharing a name with one of agy's seven builtins overrides it
  (global discovery ranks above builtins). Slice 1 lists the builtin names and
  compares.
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
`HarnessKind::ALL` and the directory it declares for each scope asserts: the
skill's and the Graph's `<dir>/<name>/SKILL.md` exist;
frontmatter `name` equals the folder name; `description` is a non-empty string;
the Graph export carries `disable-model-invocation: true`.

What it catches: a rendering a harness would drop. A harness that declares
nothing doesn't compile. What it cannot catch: a wrong path. The export writes wherever the
declaration says and the test reads the same place, so `.gemini/skills` would
pass. Only the real binary knows where it reads; that is the run below.

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

- `enum Vendor` in `skills/mod.rs`, the `vendor` argument of
  `render_graph_skill`, and the branch that leaves `disable-model-invocation`
  out of `.agents/skills`.
- The literal pair in `sync_skills` and the two literal directory strings in
  `run_sync_skills`.

Survives: every existing export, marker, collision, prune and symlink test in
`skills::tests`. Files already in `~/.claude/skills` are byte-identical after the
change; in `~/.agents/skills` skill exports are identical and each Graph export
gains the one line.

## Built

- `Capabilities::installed_skills` on all five launchers, one path per scope
  (a list returns with the first harness that needs two);
  `skills::installed_skill_directories` feeds `sync_skills` and the confirmation
  and refusal text in `run_sync_skills`; `Vendor` is gone;
  `harness::conformance_tests::every_harness_reads_a_synced_skill`.
- Docs: `docs/config.md` "Skill and Graph discovery" and the Antigravity
  sentence, `rust/loopflow/tests/README.md`, `scripts/benchmarks/pi/README.md`.
  `docs/config.md` names only Claude Code and Pi as honoring
  `disable-model-invocation`; Codex and Antigravity list the skill, nothing more
  was checked.

## Remaining

Gate. QA ran on the real binaries; `scratch/qa.html` has the transcripts.

## What the real run showed (2026-11-21, branch `lf`, this Machine's home)

- `lf self sync-skills --yes`: "synced skills (124 written, 0 pruned, 14 skipped)";
  `~/.gemini/config/skills` holds 109 exports.
- `agy -p "/skills"` (1.5.14): 7 before, 116 after. Every one of the 109
  exports is listed, Graph recipes with input files included; no export shares a
  name with a builtin (`agy-customizations`, `antigravity-guide`, `automation`,
  `migrate-agent-settings`, `migrate-workflows`, `run-workflow`, `swarm`).
- Codex 0.170.0, app-server `skills/list`: 124 skills, 0 errors; `code` and
  `pursue` from `~/.agents/skills` are `enabled: true` with
  `disable-model-invocation: true` in their frontmatter. Listing is all this
  shows; whether Codex honors the key is unchecked, so `docs/config.md` still
  names only Claude Code and Pi.
- Not run: the interactive confirmation prompt (needs a terminal), `/skills
  reload` in an open agy session, opencode and pi listings.

## Checks

- `cargo test -p loopflow-execution --lib -- skills::tests every_harness_reads_a_synced_skill`: 20 passed, rerun on f5008195ed after the one-path-per-scope change (realign).
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed (implement).
- clippy and the suites: gate and CI.
- compress: same 20 passed after rendering skills once per sync; `cargo check -p loopflow --bin lf` clean.
- qa: fresh, returning and obstructed homes; PTY prompt; agy `/skills` and `/skills reload`; `--repo` in agy, opencode, pi. Same 20 tests passed after naming the directory in sync io errors; `cargo clippy -p loopflow-execution --all-targets -- -D warnings` clean. Pi answering from a skill not run here (no working Pi model login); a person with one runs `pi -p --approve '<ask>'` in a synced repository.
