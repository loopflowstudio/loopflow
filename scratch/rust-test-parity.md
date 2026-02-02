# Rust Test Suite: User Behavior Parity

Design doc for achieving test parity between Python and Rust `lf` implementations, focusing on core `lf` user behaviors.

## Goal

Test the same user behaviors in Rust that Python tests, proving the Rust CLI can replace Python with confidence. Focus on `lf` core functionality: context assembly, prompt building, agent launching, and flow execution.

## Current State

| Metric | Python | Rust | Gap |
|--------|--------|------|-----|
| Test count | 623 | 101 | 522 |
| Modules tested | 99 | 8 | 91 |
| Core lf behaviors | 12+ | 6 | 6+ |

The Python suite tests user journeys for the `lf` command. The Rust suite has good coverage on config and prompt formatting but gaps in file gathering, agent launching, and step/direction loading.

## User Behaviors to Test

### Tier 1: Core `lf` Command (Must Have)

These are the behaviors users depend on when running `lf debug`, `lf implement`, etc.

#### 1. Context Assembly
**What users expect:** `lf debug` gathers README, CLAUDE.md, diff, and step content into a coherent prompt.

| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Include root docs (README, CLAUDE.md, STYLE.md) | ✅ | ✅ | - |
| Include .lf/docs/* | ✅ | ✅ | - |
| **Exclude gitignored files** | ✅ | ❌ | High |
| **Exclude .lf/ from file gathering** | ✅ | ❌ | High |
| Respect token budget, drop in priority order | ✅ | ✅ | - |
| **Include clipboard with -c flag** | ✅ | ⚠️ | High |
| Include diff with -d flag | ✅ | ✅ | - |
| **Include specific files with -f flag** | ✅ | ❌ | High |
| **Handle binary files gracefully** | ✅ | ❌ | Medium |
| **Deduplicate file requests** | ✅ | ❌ | Medium |

**Tests to add:**
```rust
// File exclusion
#[test]
fn gather_files_excludes_gitignored() {
    let repo = init_repo();
    write_file(repo, "src/main.rs", "fn main() {}");
    write_file(repo, "target/debug/main", "binary");
    write_file(repo, ".gitignore", "target/\n*.log");
    write_file(repo, "debug.log", "log content");

    let files = gather_files(repo, &["src/main.rs", "target/debug/main", "debug.log"]);

    assert!(files.iter().any(|f| f.path.ends_with("main.rs")));
    assert!(!files.iter().any(|f| f.path.contains("target")));
    assert!(!files.iter().any(|f| f.path.ends_with(".log")));
}

#[test]
fn gather_files_excludes_lf_directory() {
    let repo = init_repo();
    write_file(repo, "src/lib.rs", "pub fn foo() {}");
    write_file(repo, ".lf/config.yaml", "model: claude");
    write_file(repo, ".lf/steps/debug.md", "# Debug step");

    let files = gather_all_text_files(repo);

    assert!(files.iter().any(|f| f.path.ends_with("lib.rs")));
    assert!(!files.iter().any(|f| f.path.contains(".lf/")));
}

#[test]
fn gather_context_with_specific_files() {
    let repo = init_repo();
    write_file(repo, "src/a.rs", "mod a;");
    write_file(repo, "src/b.rs", "mod b;");
    write_file(repo, "src/c.rs", "mod c;");

    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        files: vec!["src/a.rs".into(), "src/c.rs".into()],
        ..Default::default()
    };
    let ctx = gather_context(&opts).unwrap();
    let prompt = format_prompt(&ctx);

    assert!(prompt.contains("mod a"));
    assert!(prompt.contains("mod c"));
    assert!(!prompt.contains("mod b"));
}

#[test]
fn gather_files_deduplicates_requests() {
    let repo = init_repo();
    write_file(repo, "src/main.rs", "fn main() {}");

    let files = gather_files(repo, &["src/main.rs", "src/main.rs", "./src/main.rs"]);

    let main_count = files.iter().filter(|f| f.path.ends_with("main.rs")).count();
    assert_eq!(main_count, 1);
}

#[test]
fn gather_files_skips_binary_files() {
    let repo = init_repo();
    write_file(repo, "src/main.rs", "fn main() {}");
    write_binary(repo, "assets/image.png", &[0x89, 0x50, 0x4E, 0x47]); // PNG header

    let files = gather_files(repo, &["src/main.rs", "assets/image.png"]);

    assert!(files.iter().any(|f| f.path.ends_with("main.rs")));
    assert!(!files.iter().any(|f| f.path.ends_with("image.png")));
}
```

#### 2. Step Loading
**What users expect:** `lf debug` finds the step in .lf/steps/ or built-in steps.

| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Load step from .lf/steps/{name}.md | ✅ | ✅ | - |
| Load built-in steps (debug, implement, etc.) | ✅ | ✅ | - |
| **Step not found returns clear error** | ✅ | ⚠️ | Medium |
| **Parse frontmatter for model, interactive** | ✅ | ⚠️ | Medium |
| **Include step directions from frontmatter** | ✅ | ❌ | High |

**Tests to add:**
```rust
#[test]
fn load_step_parses_frontmatter_model() {
    let repo = init_repo();
    write_file(repo, ".lf/steps/fast.md", r#"---
model: claude:haiku
---
# Fast Step
Do it quickly.
"#);

    let step = load_step("fast", repo).unwrap();
    assert_eq!(step.model, Some("claude:haiku".to_string()));
}

#[test]
fn load_step_parses_frontmatter_interactive() {
    let repo = init_repo();
    write_file(repo, ".lf/steps/design.md", r#"---
interactive: true
---
# Design Step
Design the feature.
"#);

    let step = load_step("design", repo).unwrap();
    assert_eq!(step.interactive, Some(true));
}

#[test]
fn load_step_includes_frontmatter_directions() {
    let repo = init_repo();
    write_file(repo, ".lf/steps/careful.md", r#"---
directions:
  - thorough
  - tested
---
# Careful Step
Be careful.
"#);
    write_file(repo, ".lf/directions/thorough.md", "Be thorough.");
    write_file(repo, ".lf/directions/tested.md", "Write tests.");

    let step = load_step("careful", repo).unwrap();
    assert_eq!(step.directions, vec!["thorough", "tested"]);
}

#[test]
fn load_step_not_found_error_message() {
    let repo = init_repo();

    let result = load_step("nonexistent", repo);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("nonexistent"));
    assert!(err.to_string().contains("not found"));
}
```

#### 3. Direction Loading
**What users expect:** Directions from -D flag or step frontmatter are loaded and included.

| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Load direction from .lf/directions/{name}.md | ✅ | ✅ | - |
| Load built-in directions | ✅ | ⚠️ | Medium |
| **Multiple directions combined in order** | ✅ | ✅ | - |
| **Direction not found returns clear error** | ✅ | ⚠️ | Medium |
| **Directions from step + CLI combined** | ✅ | ❌ | High |

**Tests to add:**
```rust
#[test]
fn directions_from_step_and_cli_combined() {
    let repo = init_repo();
    write_file(repo, ".lf/steps/impl.md", r#"---
directions:
  - thorough
---
# Implement
"#);
    write_file(repo, ".lf/directions/thorough.md", "Be thorough.");
    write_file(repo, ".lf/directions/fast.md", "Be fast.");

    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("impl".to_string()),
        directions: vec!["fast".to_string()], // CLI adds this
        ..Default::default()
    };
    let ctx = gather_context(&opts).unwrap();

    // Step's direction comes first, then CLI directions
    assert_eq!(ctx.directions.len(), 2);
    assert_eq!(ctx.directions[0].name, "thorough");
    assert_eq!(ctx.directions[1].name, "fast");
}

#[test]
fn load_direction_not_found_error() {
    let repo = init_repo();

    let result = load_direction("nonexistent", repo);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("nonexistent"));
}
```

#### 4. Agent Launching
**What users expect:** `lf debug` spawns Claude/Codex/Gemini with correct flags.

| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Build claude command with --print for auto | ✅ | ✅ | - |
| Build claude command with --stream for stream | ✅ | ✅ | - |
| **Include --dangerously-skip-permissions for auto** | ✅ | ✅ | - |
| **Include --model flag when specified** | ✅ | ❌ | High |
| **Build codex command** | ✅ | ✅ | - |
| **Build gemini command** | ✅ | ✅ | - |
| **Pass prompt via stdin** | ✅ | ❌ | High |
| **Handle model variants (claude:opus)** | ✅ | ❌ | High |

**Tests to add:**
```rust
#[test]
fn build_claude_command_with_model_variant() {
    let config = LaunchConfig {
        model: Some("opus".to_string()),
        auto: true,
        ..Default::default()
    };
    let cmd = build_claude_command(&config);

    assert!(cmd.contains(&"--model".to_string()));
    assert!(cmd.contains(&"opus".to_string()));
}

#[test]
fn build_claude_command_with_chrome_flag() {
    let config = LaunchConfig {
        chrome: true,
        ..Default::default()
    };
    let cmd = build_claude_command(&config);

    assert!(cmd.contains(&"--chrome".to_string()));
}

#[test]
fn build_claude_command_with_no_chrome_flag() {
    let config = LaunchConfig {
        no_chrome: true,
        ..Default::default()
    };
    let cmd = build_claude_command(&config);

    assert!(cmd.contains(&"--no-chrome".to_string()));
}

#[test]
fn build_codex_command_with_model() {
    let config = LaunchConfig {
        model: Some("o3".to_string()),
        ..Default::default()
    };
    let cmd = build_codex_command(&config);

    assert!(cmd.contains(&"--model".to_string()));
    assert!(cmd.contains(&"o3".to_string()));
}
```

#### 5. Prompt Formatting
**What users expect:** The assembled prompt has correct structure and ordering.

| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Include loopflow doc section | ✅ | ✅ | - |
| Include run mode for auto | ✅ | ✅ | - |
| Include wave context | ✅ | ✅ | - |
| Include docs with proper tags | ✅ | ✅ | - |
| **CLAUDE.md gets special "follow carefully" note** | ✅ | ⚠️ | Medium |
| **STYLE.md gets special "follow carefully" note** | ✅ | ⚠️ | Medium |
| Include directions in order | ✅ | ✅ | - |
| Include step content | ✅ | ✅ | - |
| Include diff when present | ✅ | ✅ | - |
| Include clipboard when present | ✅ | ✅ | - |
| **Correct section ordering** | ✅ | ✅ | - |

**Tests to add:**
```rust
#[test]
fn format_prompt_claude_md_gets_follow_note() {
    let components = PromptComponents {
        docs: vec![Document {
            path: "CLAUDE.md".to_string(),
            content: "# Instructions".to_string(),
            category: "docs".to_string(),
        }],
        ..Default::default()
    };
    let prompt = format_prompt(&components);

    assert!(prompt.contains("CLAUDE"));
    assert!(prompt.contains("Follow") || prompt.contains("carefully"));
}

#[test]
fn format_prompt_style_md_gets_follow_note() {
    let components = PromptComponents {
        docs: vec![Document {
            path: "STYLE.md".to_string(),
            content: "# Style Guide".to_string(),
            category: "docs".to_string(),
        }],
        ..Default::default()
    };
    let prompt = format_prompt(&components);

    assert!(prompt.contains("STYLE"));
    assert!(prompt.contains("Follow") || prompt.contains("carefully"));
}
```

#### 6. Token Budget Management
**What users expect:** Large contexts are trimmed intelligently to fit model limits.

| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Count tokens accurately | ✅ | ✅ | - |
| Drop summaries first | ✅ | ✅ | - |
| Drop docs next | ✅ | ✅ | - |
| Drop diff after docs | ✅ | ✅ | - |
| Drop diff_files last | ✅ | ✅ | - |
| **Never drop step or directions** | ✅ | ✅ | - |
| **Respect per-model budgets from config** | ✅ | ❌ | Medium |

**Tests to add:**
```rust
#[test]
fn trim_context_respects_model_budget() {
    let config = Config {
        budgets: BudgetConfig {
            claude: 100000,
            codex: 50000,
            gemini: 80000,
        },
        ..Default::default()
    };

    let components = make_large_components(); // 75000 tokens

    let trimmed_claude = trim_context(components.clone(), config.budgets.claude);
    let trimmed_codex = trim_context(components.clone(), config.budgets.codex);

    // Claude keeps more content
    assert!(analyze_tokens(&trimmed_claude) > analyze_tokens(&trimmed_codex));
}

#[test]
fn trim_context_never_drops_step() {
    let components = PromptComponents {
        step: Some(Step {
            name: "implement".to_string(),
            content: Some("Implement the feature with comprehensive tests".to_string()),
            ..Default::default()
        }),
        docs: vec![make_large_doc()], // 50000 tokens
        ..Default::default()
    };

    let trimmed = trim_context(components, 100); // Tiny budget

    assert!(trimmed.step.is_some());
    assert!(trimmed.docs.is_empty());
}
```

### Tier 2: Flow Execution (Should Have)

#### 7. Flow Parsing and Execution
**What users expect:** `lf flow ship` runs defined workflows.

| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Parse flow YAML | ✅ | ✅ | - |
| Execute steps in sequence | N/A | ✅ | - |
| Pause on interactive step | N/A | ✅ | - |
| Resume after interactive | N/A | ✅ | - |
| Fork into parallel branches | N/A | ✅ | - |
| **Load flow from .lf/flows/{name}.yaml** | ✅ | ✅ | - |
| **Flow not found returns clear error** | ✅ | ⚠️ | Medium |

(Flow execution is well-tested in Rust.)

### Tier 3: Edge Cases (Nice to Have)

#### 8. Config Edge Cases
| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Empty config file | ✅ | ✅ | - |
| Whitespace-only config | ✅ | ✅ | - |
| Invalid YAML syntax | ✅ | ✅ | - |
| Unknown keys ignored | ✅ | ✅ | - |
| **Environment variable expansion** | ✅ | ❌ | Low |

#### 9. Error Messages
| Behavior | Python | Rust | Priority |
|----------|--------|------|----------|
| Step not found includes searched paths | ✅ | ❌ | Medium |
| Direction not found includes searched paths | ✅ | ❌ | Medium |
| Config parse error includes line number | ✅ | ⚠️ | Low |

## Implementation Plan

### Phase 1: File Gathering (3-5 tests)

Add file filtering tests to `prompt.rs`:

```rust
mod file_gathering_tests {
    #[test] fn excludes_gitignored_files() { ... }
    #[test] fn excludes_lf_directory() { ... }
    #[test] fn includes_specific_files_flag() { ... }
    #[test] fn deduplicates_file_requests() { ... }
    #[test] fn skips_binary_files() { ... }
}
```

**Implementation notes:**
- May need to add `gather_files()` function if not present
- Use `ignore` crate for gitignore handling (same as ripgrep)

### Phase 2: Step/Direction Loading (5-7 tests)

Add frontmatter and combination tests to `flow.rs`:

```rust
mod step_loading_tests {
    #[test] fn parses_frontmatter_model() { ... }
    #[test] fn parses_frontmatter_interactive() { ... }
    #[test] fn parses_frontmatter_directions() { ... }
    #[test] fn step_not_found_error_message() { ... }
}

mod direction_tests {
    #[test] fn step_and_cli_directions_combined() { ... }
    #[test] fn direction_not_found_error() { ... }
    #[test] fn multiple_directions_ordered() { ... }
}
```

### Phase 3: Agent Launching (4-6 tests)

Extend `agent.rs` tests:

```rust
mod agent_tests {
    #[test] fn claude_with_model_variant() { ... }
    #[test] fn claude_with_chrome_flag() { ... }
    #[test] fn claude_with_no_chrome_flag() { ... }
    #[test] fn codex_with_model() { ... }
    #[test] fn gemini_with_model() { ... }
}
```

### Phase 4: Prompt Polish (3-4 tests)

Add special doc handling tests:

```rust
mod prompt_formatting_tests {
    #[test] fn claude_md_gets_follow_note() { ... }
    #[test] fn style_md_gets_follow_note() { ... }
    #[test] fn trim_respects_model_budget() { ... }
    #[test] fn trim_never_drops_step() { ... }
}
```

## Testing Patterns

### Pattern: Real Filesystem
Continue using `tempfile::TempDir` with real file operations:

```rust
fn init_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".lf/steps")).unwrap();
    std::fs::create_dir_all(dir.path().join(".lf/directions")).unwrap();
    dir
}

fn write_file(repo: &Path, path: &str, content: &str) {
    let full_path = repo.join(path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full_path, content).unwrap();
}
```

### Pattern: Test User Commands
Write tests that mirror actual `lf` invocations:

```rust
#[test]
fn user_runs_lf_debug_with_clipboard() {
    let repo = init_repo();
    write_file(repo, "README.md", "# My Project");
    write_file(repo, ".lf/steps/debug.md", "# Debug\nFix the bug.");

    // Simulates: lf debug -c
    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("debug".to_string()),
        clipboard: true,
        ..Default::default()
    };

    // Mock clipboard content
    let mut ctx = gather_context(&opts).unwrap();
    ctx.clipboard = Some("Error: undefined is not a function".to_string());

    let prompt = format_prompt(&ctx);

    assert!(prompt.contains("# My Project"));
    assert!(prompt.contains("Fix the bug"));
    assert!(prompt.contains("undefined is not a function"));
}
```

## Success Criteria

| Metric | Current | Target |
|--------|---------|--------|
| Test count | 101 | 140+ |
| File gathering tests | 0 | 5+ |
| Step loading tests | 3 | 7+ |
| Agent launching tests | 4 | 10+ |
| Core `lf` behaviors covered | 60% | 90% |

## Open Questions

| Question | Options | Recommendation |
|----------|---------|----------------|
| Test actual agent spawning? | No (side effect), stub | Stub - test command building only |
| Test clipboard reading? | Platform-specific, mock | Mock clipboard content |
| Add integration test for full `lf debug`? | Yes (spawns process), no | No - unit tests sufficient |
