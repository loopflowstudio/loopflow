# Polish Priorities

## Priority 1: Docs describe flags that don't exist in code

The `docs/lf.md` reference documents CLI flags that don't match the actual Cli struct in `rust/loopflow/src/lf/mod.rs`.

**Evidence**:
- `docs/lf.md:78` says `-a, --auto` for auto mode. Code has `-a` mapped to `--area` (`mod.rs:30`), and headless mode is `-b, --batch` (`mod.rs:50`). A user reading the docs and typing `lf review -a` expects auto mode but gets an area scope error.
- `docs/lf.md:55` says `-w, --worktree NAME` — "Create worktree and run step there." Code has `-w, --wave` (`mod.rs:66`). A user typing `lf review -w my-feature` expecting a worktree gets wave scoping instead.
- `docs/lf.md:57-58` documents `--diff-files / --no-diff-files` and `--diff / --no-diff` as CLI flags. These aren't in the Cli struct — they're config-only options. Users will get "unexpected argument" errors.
- `docs/lf.md:70-71` documents `--summaries / --no-summaries` as CLI flags. Same issue — config-only.

**Impact**: Users following the docs hit errors on their first commands. The reference is the primary onboarding surface for `lf` — wrong flags destroy trust immediately.
**Effort**: Low
**Recommendation**: Audit `docs/lf.md` against `mod.rs`. Remove flags that don't exist in CLI, add flags that do exist but aren't documented (like `-b, --batch`). Clearly distinguish CLI flags from config-only options.

## Priority 2: Docs reference commands that don't exist

`docs/lfops.md` documents three `lf ops` subcommands that aren't implemented.

**Evidence**:
- `docs/lfops.md:32-39` documents `lf ops add` with examples and flags. No `Add` variant exists in `OpsCommand` (`mod.rs:107-182`).
- `docs/lfops.md:103-109` documents `lf ops version`. No `Version` variant exists. (Version is on `lf --version` via clap.)
- `docs/lfops.md:111-121` documents `lf ops summarize` with three example invocations and a description. No `Summarize` variant exists. Referenced in `docs/config.md:141`, `docs/config.md:330`, and `docs/troubleshooting.md:101` too.

**Impact**: Users run documented commands and get "error: unrecognized subcommand" with no explanation. The ops docs are the workflow guide — phantom commands break the guided path.
**Effort**: Low (remove docs) or Medium (implement the commands)
**Recommendation**: Decide: implement these or remove them. If removed, also update `config.md` and `troubleshooting.md` references to `lf ops summarize`.

## Priority 3: 11 builtin steps invisible in `lf --list`

`BUILTIN_CATEGORIES` in `discovery.rs:34-44` only lists 17 of 28 builtin steps. The missing 11 steps exist as `.md` files and work when invoked, but users can't discover them.

**Evidence**:
- Missing from listing: `compress`, `gate`, `5whys`, `ingest`, `kickoff`, `wave-plan`, `add-to-wave`, `consolidate`, `synthesize`, `update-wave`, `validate`
- These steps are documented in README.md tables (lines 29-75) but hidden from `lf --list` output
- `builtin_descriptions()` in `discovery.rs:46-66` also lacks entries for these 11 steps, so even if categorized they'd show with blank descriptions
- Example: README says `gate` is "Ship-ready code and reviewer-friendly docs" but `lf --list` doesn't show it. A user reading the README tries `lf gate` and it works — but they'd never discover it from the CLI.

**Impact**: Users only discover these steps by reading README tables or guessing. The CLI listing is incomplete as a reference.
**Effort**: Low
**Recommendation**: Add all 28 builtin steps to `BUILTIN_CATEGORIES` and `builtin_descriptions()`. Group the missing ones into existing or new categories (e.g., "Ops" for consolidate/synthesize/validate, "Planning" for ingest/kickoff/wave-plan/5whys).

## Priority 4: `lf ops` subcommands lack help text

8 of 12 `OpsCommand` variants, all 6 `WtCommand` variants, and all 3 `ShellCommand` variants have no `///` doc comments, making `lf ops --help` output unhelpful.

**Evidence**:
- `mod.rs:125-167`: `Rebase`, `Push`, `Land`, `Pr`, `Sync`, `Next`, `Commit`, `Abandon` — no doc comments. Users see bare command names with no description.
- `mod.rs:186-223`: `Create`, `Switch`, `List`, `Prune`, `Remove`, `Ci` — no doc comments.
- `mod.rs:228-237`: `Init`, `Install`, `Directive` — no doc comments.
- Positional args also lack `help` attributes: `Rebase { onto }`, `Abandon { branch }`, `Create { name }` — users don't know what to pass.
- Contrast with the 4 documented variants (`Cp`, `Doctor`, `Wt`, `Shell`) that do have `///` doc comments and show clear descriptions in `--help`.

**Impact**: `lf ops --help` shows a wall of undescribed commands. Users must read source code or docs to understand what each command does.
**Effort**: Low
**Recommendation**: Add one-line `///` doc comments to every OpsCommand, WtCommand, and ShellCommand variant. Add `help` attributes to positional args.

## Priority 5: Python CLI (`lfq`) commands lack help text

Most `lfq` commands have no `help` parameter in their typer decorators.

**Evidence**:
- `cli.py:66`: `@app.command("list")` — no help. User sees `list` with no description.
- `cli.py:81`: `@app.command("show")` — no help.
- `cli.py:105`: `@app.command("create")` — no help.
- `cli.py:117-133`: `run`, `stop`, `delete`, `land` — no help.
- `cli.py:137`: `logs` — no help.
- Only the top-level app has help: `"Query lfd and manage waves."` (`cli.py:15`)

**Impact**: `lfq --help` shows command names but no descriptions. Users must guess or read source.
**Effort**: Low
**Recommendation**: Add `help="..."` to each `@app.command()` decorator.

## Priority 6: Error messages don't guide users

Error messages across both CLIs tell users what went wrong but not how to fix it.

**Evidence**:
- `cli.py:85`: `typer.echo(f"wave not found: {name_or_id}", err=True)` — doesn't suggest `lfq list` to see available waves.
- `discovery.rs:24`: `"step or flow not found: {name}"` — doesn't suggest `lf --list` to see available options, or note the search paths tried.
- `run.rs:209`: `"'{}' CLI not found"` — doesn't suggest how to install the backend.
- `run.rs:267`: `"agent exited with code {}"` — no guidance on what the exit code means or where to look for logs.

**Impact**: Users hit errors and have to search docs or source code for next steps. Fixable errors become dead ends.
**Effort**: Medium
**Recommendation**: Add actionable hints to error messages. "wave not found: X. Run `lfq list` to see available waves." "step or flow not found: X. Run `lf --list` to see available steps."

## Lower priority

**`-w` flag reused for three different things.** In the main CLI, `-w` is `--wave` (`mod.rs:66`). In `Land`, `-w` is `--worktree` (`mod.rs:139`). In `Ci`, `-w` is `--watch` (`mod.rs:219`). Not a bug since they're in different subcommand scopes, but adds cognitive load when switching between commands.

**Inconsistent error message capitalization.** Rust errors mix styles: `"no step specified"` (lowercase, `run.rs`), `"CLI fork execution only supports..."` (sentence case, `flow.rs:89`). Minor but contributes to an unpolished feel.

**README step categories don't match code categories.** README groups steps as "Planning steps (plan/)", "Code steps (code/)", etc. — matching the directory structure in `builtins/steps/`. But `BUILTIN_CATEGORIES` uses different names: "Setup", "Planning & Design", "Implementation", "Quality", "Scan", "Git". Neither is wrong, but users switching between README and `lf --list` see different groupings.

**Swift `getWave()` naming.** `swift/LoopflowCore/Services/WaveServiceProtocol.swift:17` and `LocalWaveService.swift:138` use `getWave()`. CLAUDE.md says no `get_` prefix on getters. Should be `wave()`.
