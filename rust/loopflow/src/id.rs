use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid id: {0}")]
pub struct IdError(String);

/// Fold only a structured UUID's ASCII case, retaining its accepted punctuation.
pub(crate) fn canonical_uuid(value: &str) -> String {
    if Uuid::parse_str(&value.to_ascii_lowercase()).is_ok() {
        value.to_ascii_lowercase()
    } else {
        value.to_owned()
    }
}

/// Canonicalize only a recognized prefixed UUID; historical values remain intact.
pub(crate) fn canonical_prefixed(value: &str, prefix: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if lower
        .strip_prefix(prefix)
        .is_some_and(|suffix| Uuid::parse_str(suffix).is_ok())
    {
        lower
    } else {
        value.to_owned()
    }
}

pub(crate) fn canonical_session(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let hex = lower.strip_prefix("session_").unwrap_or(&lower);
    if hex.len() == 32 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        lower
    } else {
        value.to_owned()
    }
}

pub(crate) fn canonical_task_identifier(value: &str) -> String {
    if let Some((team, number)) = value.rsplit_once('-') {
        if team.eq_ignore_ascii_case("lf")
            && !number.is_empty()
            && number.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return value.to_ascii_lowercase();
        }
        if !team.is_empty()
            && team.bytes().all(|byte| byte.is_ascii_alphanumeric())
            && !number.is_empty()
            && number.bytes().all(|byte| byte.is_ascii_digit())
        {
            return value.to_ascii_uppercase();
        }
    }
    value.to_owned()
}

/// One Task selector spelled in each identity domain it may name. A spelling
/// outside a domain stays intact, so an opaque external ID is never a ticket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskSelector {
    pub id: String,
    pub external: String,
    pub label: String,
    /// An abbreviated stable Task ID, with or without `lf-`/`task_`.
    pub local_prefix: Option<String>,
}

impl TaskSelector {
    pub fn parse(value: &str) -> Self {
        let lower = value.to_ascii_lowercase();
        let hex = lower
            .strip_prefix("lf-")
            .or_else(|| lower.strip_prefix("task_"))
            .unwrap_or(&lower);
        Self {
            id: canonical_prefixed(value, "task_"),
            external: canonical_uuid(value),
            label: canonical_task_identifier(value),
            local_prefix: ((4..=32).contains(&hex.len())
                && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| hex.to_owned()),
        }
    }

    pub fn matches(&self, value: &str) -> bool {
        value == self.id || value == self.external || value == self.label
    }
}

pub(crate) fn deserialize_uuid<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let value = <String as serde::Deserialize>::deserialize(deserializer)?;
    Ok(canonical_uuid(&value))
}

/// Historical follow-through references name a local Task or a Linear issue.
pub(crate) fn deserialize_task_or_uuid<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let value = <String as serde::Deserialize>::deserialize(deserializer)?;
    Ok(canonical_prefixed(&canonical_uuid(&value), "task_"))
}

pub(crate) fn deserialize_uuid_option<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let value = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
    Ok(value.map(|value| canonical_uuid(&value)))
}

pub(crate) fn deserialize_uuid_vec<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    let values = <Vec<String> as serde::Deserialize>::deserialize(deserializer)?;
    let mut seen = std::collections::BTreeMap::new();
    values
        .into_iter()
        .map(|value| {
            let canonical = canonical_uuid(&value);
            if let Some(original) = seen.insert(canonical.clone(), value.clone()) {
                if original != value {
                    return Err(serde::de::Error::custom(format!(
                        "canonical identity collision: {original} / {value}"
                    )));
                }
            }
            Ok(canonical)
        })
        .collect()
}

pub(crate) fn deserialize_task_identifier<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let value = <String as serde::Deserialize>::deserialize(deserializer)?;
    Ok(canonical_task_identifier(&value))
}

macro_rules! prefixed_uuid_id {
    ($name:ident, $prefix:literal, $error:ty, $invalid:path) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, uuid::Uuid::new_v4().simple()))
            }

            pub fn parse(value: &str) -> Result<Self, $error> {
                let canonical = value.to_ascii_lowercase();
                let suffix = canonical
                    .strip_prefix($prefix)
                    .ok_or_else(|| $invalid(format!("expected {} id", $prefix)))?;
                uuid::Uuid::parse_str(suffix).map_err(|error| $invalid(error.to_string()))?;
                Ok(Self(canonical))
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

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = $error;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                // Released serde accepted historical strings; normalize without narrowing that contract.
                Ok(Self(crate::id::canonical_prefixed(&value, $prefix)))
            }
        }
    };
}
pub(crate) use prefixed_uuid_id;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4().to_string())
            }

            pub fn parse(value: &str) -> Result<Self, IdError> {
                Uuid::parse_str(&value.to_ascii_lowercase())
                    .map_err(|error| IdError(error.to_string()))?;
                Ok(Self(value.to_ascii_lowercase()))
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
uuid_id!(SteerId);
uuid_id!(SteerBatchId);

// One claim, not a Process identity. Never reused after attachment transfer.
uuid_id!(AttachmentToken);

/// Loopflow-owned conversation identity. Native provider conversations use AgentSessionId.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct LfSessionId(String);

impl LfSessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl From<String> for LfSessionId {
    fn from(value: String) -> Self {
        Self(canonical_session(&value))
    }
}
impl From<&str> for LfSessionId {
    fn from(value: &str) -> Self {
        Self(canonical_session(value))
    }
}
impl From<LfSessionId> for String {
    fn from(value: LfSessionId) -> Self {
        value.0
    }
}
impl PartialEq<str> for LfSessionId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}
impl PartialEq<&str> for LfSessionId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}
impl PartialEq<String> for LfSessionId {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}
impl std::ops::Deref for LfSessionId {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Display for LfSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
impl<'de> serde::Deserialize<'de> for LfSessionId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self::from(value))
    }
}
impl rusqlite::ToSql for LfSessionId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}
impl rusqlite::types::FromSql for LfSessionId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Ok(Self::from(value.as_str()?))
    }
}

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

#[cfg(test)]
mod canonical_tests {
    use super::{AgentSessionId, AttachmentToken, LfSessionId, TaskSelector, WaveId};
    use crate::durable::{MachineId, ProjectId, TaskId};
    use crate::linear::LinearIssueId;
    use crate::pr_landing::PrLandingId;
    use crate::work::task::TaskPrId;
    use std::collections::HashMap;

    #[test]
    fn canonical_ids_constructors_serde_hash_and_sql_agree() {
        let uuid = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
        for original in [
            uuid.to_owned(),
            uuid.replace('-', ""),
            format!("{{{uuid}}}"),
            format!("URN:UUID:{uuid}"),
        ] {
            let canonical = original.to_ascii_lowercase();
            let id = WaveId::parse(&original).unwrap();
            let decoded: WaveId = serde_json::from_value(original.clone().into()).unwrap();
            assert_eq!(id, decoded);
            assert_eq!(id.as_str(), canonical);
            assert_eq!(
                HashMap::from([(id.clone(), 1)]).get(&WaveId::parse(&canonical).unwrap()),
                Some(&1)
            );
            assert_eq!(serde_json::to_value(&id).unwrap(), canonical);
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            let stored: WaveId = conn.query_row("SELECT ?1", [&id], |r| r.get(0)).unwrap();
            assert_eq!(stored, id);
            assert_eq!(
                AttachmentToken::parse(&original).unwrap().as_str(),
                canonical
            );
            assert_eq!(LinearIssueId::new(&original).unwrap().as_str(), canonical);
        }
        macro_rules! prefixed {
            ($ty:ty,$prefix:literal) => {{
                let value = format!("{}{}", $prefix, uuid);
                let id = <$ty>::parse(&value).unwrap();
                assert_eq!(id.as_str(), value.to_ascii_lowercase());
                assert_eq!(serde_json::from_value::<$ty>(value.into()).unwrap(), id);
            }};
        }
        prefixed!(TaskId, "TASK_");
        prefixed!(ProjectId, "PROJ_");
        prefixed!(MachineId, "HOME_");
        prefixed!(TaskPrId, "PR_");
        prefixed!(PrLandingId, "LANDING_");
        assert_eq!(
            TaskId::from_raw("TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").as_str(),
            "task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            LinearIssueId::from_raw("Case-Sensitive-History").as_str(),
            "Case-Sensitive-History"
        );
    }

    #[test]
    fn canonical_ids_session_is_distinct_from_native_conversation() {
        for value in [
            "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            let id: LfSessionId = serde_json::from_value(value.into()).unwrap();
            assert_eq!(id.as_str(), value.to_ascii_lowercase());
            assert_ne!(
                AgentSessionId::from(value),
                AgentSessionId::from(value.to_ascii_lowercase())
            );
        }
        for value in [
            "Historical-Conversation",
            "run_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
        ] {
            assert_eq!(LfSessionId::from(value).as_str(), value);
        }
    }

    #[test]
    fn canonical_ids_task_selector_keeps_each_domain_separate() {
        let ticket = TaskSelector::parse("lOo-506");
        assert_eq!(ticket.label, "LOO-506");
        assert_eq!(ticket.external, "lOo-506");
        let opaque = TaskSelector::parse("issue-1");
        assert!(opaque.matches("issue-1"));
        assert_eq!(opaque.external, "issue-1");
        let task = TaskSelector::parse("TASK_ABCDEF");
        assert_eq!(task.local_prefix.as_deref(), Some("abcdef"));
        let linear = TaskSelector::parse("FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF");
        assert!(linear.matches("ffffffff-ffff-4fff-8fff-ffffffffffff"));
        assert_eq!(TaskSelector::parse("lf-ABCD").label, "lf-abcd");
    }
}
