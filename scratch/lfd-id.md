# LfdId: Unified ID Type

## Problem

Entity IDs in lfd are scattered `String` fields with no type safety or centralized handling. Different backends (SQLite, Postgres) could benefit from different storage strategies, but the current approach hardcodes TEXT everywhere.

## Approach

Introduce a single `LfdId` newtype used for all entity identifiers. Centralize ID generation, validation, and serialization in one place.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LfdId(String);

impl LfdId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn parse(s: &str) -> Result<Self, IdError> {
        uuid::Uuid::parse_str(s)?;
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

## Scope

**In scope:**
- `LfdId` newtype in `rust/lfd/src/id.rs`
- Serialization traits: `Display`, `FromStr`, `Serialize`, `Deserialize`
- SQLite support via `rusqlite::ToSql` / `FromSql` (TEXT)
- Postgres support via `tokio_postgres::ToSql` / `FromSql` (TEXT initially)
- Update store methods to use `&LfdId` instead of `&str`
- Update proto boundary to convert `String` <-> `LfdId`

**Out of scope (future):**
- Postgres native UUID storage (migration + `ToSql` change)
- Per-entity ID types (`WaveId`, `StepRunId`, etc.)

## Implementation

### 1. Create id.rs

```rust
// rust/lfd/src/id.rs

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LfdId(String);

#[derive(Debug, thiserror::Error)]
#[error("invalid id: {0}")]
pub struct IdError(String);

impl LfdId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for LfdId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LfdId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for LfdId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map_err(|e| IdError(e.to_string()))?;
        Ok(Self(s.to_string()))
    }
}

impl From<LfdId> for String {
    fn from(id: LfdId) -> String {
        id.0
    }
}

// Serde: serialize as string
impl serde::Serialize for LfdId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for LfdId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}
```

### 2. SQLite support

```rust
impl rusqlite::ToSql for LfdId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.0.as_str()))
    }
}

impl rusqlite::types::FromSql for LfdId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse().map_err(|e| rusqlite::types::FromSqlError::Other(Box::new(e)))
    }
}
```

### 3. Postgres support (TEXT for now)

```rust
impl tokio_postgres::types::ToSql for LfdId {
    fn to_sql(
        &self,
        ty: &tokio_postgres::types::Type,
        out: &mut bytes::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        self.0.to_sql(ty, out)
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        <String as tokio_postgres::types::ToSql>::accepts(ty)
    }

    tokio_postgres::types::to_sql_checked!();
}

impl<'a> tokio_postgres::types::FromSql<'a> for LfdId {
    fn from_sql(
        ty: &tokio_postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let s = <String as tokio_postgres::types::FromSql>::from_sql(ty, raw)?;
        Ok(s.parse()?)
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        <String as tokio_postgres::types::FromSql>::accepts(ty)
    }
}
```

### 4. Update store trait

Change method signatures from `&str` to `&LfdId`:

```rust
// Before
fn get_wave(&self, wave_id: &str) -> StoreResult<Option<Wave>>;

// After
fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>>;
```

### 5. Proto boundary

Proto types keep `String` fields. Convert at the service layer:

```rust
// Incoming: String -> LfdId
let wave_id: LfdId = request.wave_id.parse()?;

// Outgoing: LfdId -> String
response.wave_id = wave.id.to_string();
```

## Migration path to native UUID (future)

When ready to optimize Postgres storage:

1. Schema migration: `ALTER COLUMN id TYPE UUID USING id::uuid`
2. Update `ToSql`/`FromSql` to use `uuid::Uuid` instead of `String`
3. No Rust API changes—`LfdId` interface stays the same

## Files changed

| File | Change |
|------|--------|
| `rust/lfd/src/id.rs` | New: LfdId type |
| `rust/lfd/src/lib.rs` | Add `mod id; pub use id::LfdId;` |
| `rust/lfd/src/store/mod.rs` | Update trait signatures |
| `rust/lfd/src/store/sqlite.rs` | Use LfdId in queries |
| `rust/lfd/src/store/postgres.rs` | Use LfdId in queries |
| `rust/lfd/src/server.rs` | Convert at proto boundary |
| `rust/lfd/Cargo.toml` | Add `uuid` dependency if not present |

## Done when

- [x] `LfdId::new()` generates valid UUIDs
- [x] `LfdId::parse()` validates UUID format
- [x] SQLite store compiles and tests pass
- [x] Postgres store compiles and tests pass
- [x] No raw `&str` IDs in store trait methods
- [x] `cargo clippy` clean
