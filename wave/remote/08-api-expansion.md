# 08: API Expansion

File browsing and step/flow/direction metadata via lfd API. Server-side support for Concerto features that need remote file access.

## What exists after this

Concerto can browse files in a wave's worktree, read file contents, view diffs, and search/typeahead across steps, flows, and directions — all through lfd's existing HTTP API. No filesystem mount.

## Context

lfd already serves wave data, logs, and events. Adding file and metadata endpoints extends the same pattern: lfd reads the filesystem or parses config on the remote machine, returns JSON over HTTP.

These endpoints serve both Concerto (macOS) and future mobile clients.

## Carryover from Phase 05 scope cut

Phase 05 ships with minimal remote-safe guards but does not replace local filesystem-backed UX. The following roll into this phase:

- API-backed replacement for local `AreaTypeahead` filesystem reads in remote mode
- Remote file browsing and content/diff endpoints for worktree inspection
- Remote-first typeahead parity where local disk access is currently assumed

## File browsing endpoints

```
GET /v0/waves/{wave_id}/files?path=src/
→ {
    "path": "src/",
    "entries": [
      { "name": "main.rs", "type": "file", "size": 2340 },
      { "name": "lib/", "type": "directory" }
    ]
  }

GET /v0/waves/{wave_id}/file?path=src/main.rs
→ {
    "path": "src/main.rs",
    "content": "fn main() { ... }",
    "size": 2340,
    "language": "rust"
  }

GET /v0/waves/{wave_id}/diff
→ {
    "base": "main",
    "head": "wave-branch",
    "files_changed": 3,
    "diff": "--- a/src/main.rs\n+++ b/src/main.rs\n..."
  }
```

### Implementation (Rust)

```rust
// lfd/http/routes/files.rs

async fn list_files(
    Path(wave_id): Path<String>,
    Query(params): Query<FileListParams>,
    State(state): State<AppState>,
) -> Result<Json<FileListResponse>> {
    let wave = state.store.get_wave(&wave_id).await?;
    let worktree = infer_worktree(&wave)?;
    let relative = PathBuf::from(params.path.unwrap_or_default());
    let dir = path_within_root_existing(&worktree, &relative)?;

    let entries = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| FileEntry {
            name: e.file_name().to_string_lossy().into(),
            file_type: if e.file_type().ok()?.is_dir() { "directory" } else { "file" },
            size: e.metadata().ok()?.len(),
        })
        .collect();

    Ok(Json(FileListResponse { path: params.path, entries }))
}

async fn read_file(
    Path(wave_id): Path<String>,
    Query(params): Query<FileReadParams>,
    State(state): State<AppState>,
) -> Result<Json<FileContentResponse>> {
    let wave = state.store.get_wave(&wave_id).await?;
    let worktree = infer_worktree(&wave)?;
    let file_path = path_within_root_existing(&worktree, Path::new(&params.path))?;

    // Cap file size to prevent OOM
    let metadata = fs::metadata(&file_path)?;
    if metadata.len() > 1_000_000 {
        return Err(ApiError::FileTooLarge);
    }

    let content = fs::read_to_string(&file_path)?;
    let language = detect_language(&params.path);

    Ok(Json(FileContentResponse { path: params.path, content, size: metadata.len(), language }))
}
```

Before any `read_dir`, `metadata`, or file-content read, resolve user-supplied paths through
`path_within_root_existing`/`path_within_root_planned`. Reject traversal attempts (`..`, absolute
paths, symlink escapes, null bytes) with `400`.

## Step/flow/direction endpoints

Separate endpoints per type — steps, flows, and directions have different structures.

```
GET /v0/steps?q=des
→ {
    "results": [
      {
        "name": "design",
        "category": "interactive",
        "summary": "Interactive design session",
        "path": ".lf/steps/interactive/design.md",
        "requires": "none",
        "produces": "scratch/<branch>.md",
        "interactive": true
      }
    ]
  }

GET /v0/flows?q=ship
→ {
    "results": [
      { "name": "ship", "steps": ["implement", "compress", "gate", "consolidate"] },
      { "name": "design-and-ship", "steps": ["design", "implement", "reduce", "polish"] }
    ]
  }

GET /v0/directions?q=prod
→ {
    "results": [
      { "name": "product-engineer", "summary": "Think with product concerns" },
      { "name": "designer", "summary": "Think with design concerns" }
    ]
  }
```

### Implementation

```rust
// lfd/http/routes/steps.rs
async fn list_steps(
    Query(params): Query<SearchParams>,
    State(state): State<AppState>,
) -> Result<Json<StepListResponse>> {
    let repo = default_repo(&state)?;
    let steps_dir = repo.join(".lf/steps");
    // Walk directory, parse frontmatter + opening line from each .md file
    // Filter by query (fuzzy match on name)
    // Return name, category (subdirectory), summary, frontmatter fields
}

// lfd/http/routes/flows.rs
async fn list_flows(
    Query(params): Query<SearchParams>,
    State(state): State<AppState>,
) -> Result<Json<FlowListResponse>> {
    let repo = default_repo(&state)?;
    let flows_dir = repo.join(".lf/flows");
    // Walk directory, parse step sequence from each flow file
    // Filter by query
}

// lfd/http/routes/directions.rs
async fn list_directions(
    Query(params): Query<SearchParams>,
    State(state): State<AppState>,
) -> Result<Json<DirectionListResponse>> {
    let repo = default_repo(&state)?;
    let directions_dir = repo.join(".lf/directions");
    // Walk directory, parse opening line as summary
    // Filter by query
}
```

### Metadata parsing

Steps, flows, and directions are markdown files with different structures:

**Steps** — frontmatter (`requires`, `produces`, `interactive`) + opening line as summary. Category from parent directory (`interactive/design.md` → `interactive`).

**Flows** — sequence of step names. Category from parent directory (`code/ship.yaml` → `code`).

**Directions** — opening line as summary. No frontmatter.

Cache scan results per type. Invalidate when the corresponding `.lf/` subdirectory mtime changes.

## Concerto integration

### File browser (future view)

Not building a full file browser yet — but the API is ready when we want one. Likely a simple tree view in wave detail that shows changed files with syntax-highlighted diffs.

### Typeahead in wave config

When editing a wave's flow or direction, query the relevant endpoint for autocomplete:

```swift
TextField("Flow", text: $flowInput)
    .onChange(of: flowInput) { query in
        Task {
            suggestions = try await service.searchFlows(query: query)
        }
    }
```

## Done when

- `GET /v0/waves/{id}/files` lists directory contents
- `GET /v0/waves/{id}/file` returns file content (with size cap)
- `GET /v0/waves/{id}/diff` returns git diff for the wave's branch
- `GET /v0/steps` returns steps with frontmatter metadata
- `GET /v0/flows` returns flows with step sequences
- `GET /v0/directions` returns directions with summaries
- All three support `?q=` for typeahead filtering
- Responses are fast enough for typeahead (<100ms locally, <300ms over WAN)
