use std::fmt;
use std::str::FromStr;

use bytes::BytesMut;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LfdId(String);

#[derive(Debug, thiserror::Error)]
#[error("invalid id: {0}")]
pub struct IdError(String);

impl LfdId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn parse(value: &str) -> Result<Self, IdError> {
        Uuid::parse_str(value).map_err(|err| IdError(err.to_string()))?;
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
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

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<LfdId> for String {
    fn from(id: LfdId) -> String {
        id.0
    }
}

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
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

impl rusqlite::ToSql for LfdId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.0.as_str()))
    }
}

impl rusqlite::types::FromSql for LfdId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse()
            .map_err(|err| rusqlite::types::FromSqlError::Other(Box::new(err)))
    }
}

impl tokio_postgres::types::ToSql for LfdId {
    fn to_sql(
        &self,
        ty: &tokio_postgres::types::Type,
        out: &mut BytesMut,
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
        let value = <String as tokio_postgres::types::FromSql>::from_sql(ty, raw)?;
        Ok(value.parse()?)
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        <String as tokio_postgres::types::FromSql>::accepts(ty)
    }
}
