# Design: wave status, run snapshots, name lookup index, Swift model shrink

## What to build
Implement three simplification opportunities in lfd + clients: first-class wave status, run configuration snapshots, name-indexed wave lookup, plus shrink the Swift Wave model to match the v1 API contract.

> "lets address these 3 issues # Simplification Opportunities"
> "Also add on ... Opportunity 4: Shrink the Swift Wave model to match the API contract"

## Data structures
```rust
// rust/lfd: persist status on Wave, remove paused bool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: WaveId,
    pub name: String,
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub status: WaveStatus,
    pub iteration: i64,
    pub stimulus: Stimulus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum WaveStatus {
    Idle,
    Running,
    Waiting,
    Paused,
    Failed,
    Completed,
}

// rust/lfd: snapshot run config + PR metadata at run creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveRunSnapshot {
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub pr: Option<PullRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveRun {
    pub id: RunId,
    pub wave_id: WaveId,
    pub iteration: i64,
    pub step_index: i64,
    pub status: RunStatus,
    pub local_worktree: String,
    pub remote_branch: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub flow_parents: Vec<String>,
    pub snapshot: WaveRunSnapshot,
}

// rust/lfd store: name index for direct lookup
trait WaveStore {
    fn wave(&self, id: &WaveId) -> Option<Wave>;
    fn wave_by_name(&self, name: &str) -> Option<Wave>;
}
```

```python
# python/loopflow/models.py
class Wave(BaseModel):
    id: str
    name: str
    repo: str
    flow: str
    direction: list[str]
    area: list[str]
    status: str  # includes "paused" enum value
    iteration: int
    stimulus: Stimulus
    created_at: datetime | None = None

class WaveRunSnapshot(BaseModel):
    repo: str
    flow: str
    direction: list[str]
    area: list[str]
    pr: PullRequest | None = None

class WaveRun(BaseModel):
    ...
    snapshot: WaveRunSnapshot
```

```swift
// swift/LoopflowCore: API model only
struct WaveApi: Codable, Identifiable, Equatable {
    let id: String
    let name: String
    let repo: String
    let flow: String
    let direction: [String]
    let area: [String]
    let status: WaveStatus
    let iteration: Int
    let activeRun: WaveRun?
    let createdAt: Date?
}

// UI/view model aggregates API + derived UI state
struct WaveViewModel: Identifiable, Equatable {
    let api: WaveApi
    var staleness: Staleness?
    var statusIndicator: StatusIndicator
    var displayName: String
    var lastActivityDescription: String
}
```

## Key functions
```rust
// rust/lfd: write path updates status directly
fn set_wave_status(wave_id: WaveId, status: WaveStatus) -> Result<Wave, StoreError>;

// rust/lfd: create run snapshot when run starts
fn create_wave_run(wave: &Wave, run: NewWaveRun) -> Result<WaveRun, StoreError>;

// rust/lfd: resolve wave by id OR name using index
fn resolve_wave_id(store: &Store, handle: &str) -> Option<WaveId>;
```

```python
# python/loopflow/client.py
# no paused translation; pass status directly
Client.update_wave(name_or_id: str, *, status: str | None = None, ...) -> Wave
```

```swift
// swift/LoopflowCore/Services/LocalWaveService.swift
func parseWave(from json: [String: Any]) throws -> WaveApi
```

## Constraints
- Migration strategy is intentionally **deferred**. Don’t build a new system now; accept a best-effort fallback for existing data and avoid blocking clients today.
- PR metadata should be as live as possible: we still want to create draft PRs and ideally update description, but **don’t block run creation** on GH calls. Consider background refresh if needed.
- Name index must remain unique: **hard error** on collisions for now.
- Swift model shrink: keep API model strictly aligned with the v1 response; move UI-only state to a separate view model.

## Done when
- `uv run pytest tests/` passes.
- `cargo test --all` passes.
- `lfq show <wave>` returns status without extra run lookups (confirm via logs).
- Swift compiles with the new API model and `parseWave` only reads keys sent by the v1 API.
