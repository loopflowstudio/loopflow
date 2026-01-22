# CLI Architecture: lf vs lfops

**Status: Implemented**

## Principle

**`lf` = Prompt Launcher**

Every `lf` invocation launches a prompt. The subcommand is the prompt name:

```bash
lf review                    # run review prompt
lf implement: add auth       # run implement prompt with args
lf : "fix this bug"          # run inline prompt
lf flow ship                 # run chained prompts
```

Flags modify *where* or *how* the prompt executes:
- `--web` — execute via web client (claude.ai, chatgpt.com, aistudio.google.com)
- `--copy` — copy to clipboard, show tokens (dry run)
- `-i` / `-a` — interactive vs auto mode
- `--model` — which backend/variant

**`lfops` = Everything Else**

Operations, utilities, prompt management—even if they use prompts internally:

```bash
lfops cp [paths]             # copy context to clipboard
lfops pr / land / commit     # git workflow
lfops wt create / prune      # worktree management
lfops summarize              # generate summaries
```

## Changes

### 1. Add `--web` flag to `lf`

When `--web` is passed:
1. Assemble prompt context (same as normal)
2. Copy to clipboard
3. Open web client based on `agent_model` config

Web client mapping:
| Backend | URL |
|---------|-----|
| claude | `https://claude.ai/new` |
| codex | `https://chatgpt.com` |
| gemini | `https://aistudio.google.com/prompts/new_chat` |

Note: URL query params (e.g., `?q=prompt`) have length limits (~2-8KB), so we copy to clipboard instead of encoding in URL.

### 2. Move `cp` from `lf` to `lfops`

`lf cp` violates the "prompt launcher" principle—it copies files without launching a prompt.

Move to `lfops cp` with identical behavior:
- Takes paths as positional args
- Supports `--exclude`, `--paste`, `--lfdocs`, `--diff-files`, `--summaries`
- Shows token breakdown
- Copies to clipboard

### 3. Remove `add` from `lf`

`lf add` creates prompt files but doesn't launch them. Move to `lfops add` or `lfops prompt add`.

## Implementation

### Files to modify

1. `src/loopflow/lf/run.py`
   - Add `--web` flag to `run()`, `inline()`, `flow()`
   - Add `_open_web_client(backend: str)` helper
   - Remove `cp()` function

2. `src/loopflow/lf/__init__.py`
   - Remove `app.command()(run_module.cp)`
   - Remove `"cp"` from `known_commands`

3. `src/loopflow/lfops/cp.py` (new file)
   - Move `cp()` function from run.py
   - Register via `register_commands(app)`

4. `src/loopflow/lfops/commands.py`
   - Import and register cp module

5. `docs/lf.md` / `docs/lfops.md`
   - Update command documentation

### Web client URLs

```python
WEB_CLIENTS = {
    "claude": "https://claude.ai/new",
    "codex": "https://chatgpt.com",
    "gemini": "https://aistudio.google.com/prompts/new_chat",
}
```

### --web behavior

```python
if web:
    # Assemble and copy (same as --copy)
    prompt = format_prompt(components)
    _copy_to_clipboard(prompt)
    tree = analyze_components(components)
    _show_token_breakdown(tree)

    # Open browser
    backend, _ = parse_model(config.agent_model if config else "claude")
    url = WEB_CLIENTS.get(backend, WEB_CLIENTS["claude"])
    subprocess.run(["open", url])
    raise typer.Exit(0)
```
