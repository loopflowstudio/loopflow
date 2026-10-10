use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid id: {0}")]
pub struct IdError(String);

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4().to_string())
            }

            pub fn parse(value: &str) -> Result<Self, IdError> {
                Uuid::parse_str(value).map_err(|error| IdError(error.to_string()))?;
                Ok(Self(value.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                value.parse().map_err(serde::de::Error::custom)
            }
        }

        impl rusqlite::ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(rusqlite::types::ToSqlOutput::from(self.0.as_str()))
            }
        }

        impl rusqlite::types::FromSql for $name {
            fn column_result(
                value: rusqlite::types::ValueRef<'_>,
            ) -> rusqlite::types::FromSqlResult<Self> {
                value
                    .as_str()?
                    .parse()
                    .map_err(|error| rusqlite::types::FromSqlError::Other(Box::new(error)))
            }
        }
    };
}

uuid_id!(WaveId);
uuid_id!(TraceId);
uuid_id!(LfProcessId);

// One claim, not a Process identity. Never reused after attachment transfer.
uuid_id!(AttachmentToken);

/// A provider-owned conversation id. Opaque vendor bytes, never an lf identity.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct AgentSessionId(String);

impl AgentSessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AgentSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for AgentSessionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Display for AgentSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl rusqlite::ToSql for AgentSessionId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl rusqlite::types::FromSql for AgentSessionId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Ok(Self::from(value.as_str()?))
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentSessionId, LfProcessId, TraceId, WaveId};

    #[test]
    fn agent_session_preserves_opaque_provider_bytes_in_json_and_sql() {
        let id = AgentSessionId::from("session_vendor:Case-Sensitive/01");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""session_vendor:Case-Sensitive/01""#);
        assert_eq!(serde_json::from_str::<AgentSessionId>(&json).unwrap(), id);
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let saved: AgentSessionId = db.query_row("SELECT ?1", [&id], |row| row.get(0)).unwrap();
        assert_eq!(saved, id);
    }

    #[test]
    fn ids_round_trip_as_uuid_strings() {
        let wave = WaveId::new();
        let encoded = serde_json::to_string(&wave).unwrap();
        assert_eq!(serde_json::from_str::<WaveId>(&encoded).unwrap(), wave);

        let _trace = TraceId::new();
        let _process = LfProcessId::new();
    }
}
