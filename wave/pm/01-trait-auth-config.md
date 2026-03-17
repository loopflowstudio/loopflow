# 01: PmProvider trait, auth, and config

**Finish line:** `PmProvider` trait defined. `Asana` and `Linear` added to `Provider` enum with credential storage. `pm` block parseable in wave YAML. `pm_id` parseable in roadmap item frontmatter.

## What to build

### Trait

```rust
#[async_trait]
trait PmProvider: Send + Sync {
    async fn create_project(&self, name: &str, description: &str) -> Result<String>;
    async fn list_items(&self, project_id: &str) -> Result<Vec<PmItem>>;
    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> Result<String>;
    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> Result<()>;
    async fn complete_item(&self, item_id: &str) -> Result<()>;
    async fn comment(&self, item_id: &str, body: &str) -> Result<()>;
}
```

Types: `PmItem`, `PmItemCreate`, `PmItemUpdate` — see design doc.

### Auth

Add `Asana` and `Linear` to `Provider` enum in `providers.rs`. Both use API key/PAT auth (no OAuth needed for v1). `lfq auth asana` and `lfq auth linear` store credentials through the existing encrypted storage path.

### Config

Wave YAML: `pm.provider` (enum: `asana` | `linear`) and `pm.project` (string ID).

`.lf/config.yaml`:
```yaml
asana:
  workspace: "..."
  default_team: "..."
linear:
  team: "..."
```

Roadmap item frontmatter: `pm_id: "..."` — provider-agnostic, resolved via wave YAML.

## Constraints

- Don't implement the actual API clients yet — just the trait and types
- Auth should work end-to-end: `lfq auth asana` stores a PAT, retrievable for later use
- Wave YAML parsing must be backwards-compatible — waves without `pm` are unaffected

## Done when

- `PmProvider` trait compiles with both `PmItem` types
- `lfq auth asana` and `lfq auth linear` store and retrieve credentials
- A wave YAML with `pm:` block parses correctly
- A wave YAML without `pm:` block still parses correctly
- Roadmap `.md` with `pm_id:` frontmatter round-trips through parse/write
