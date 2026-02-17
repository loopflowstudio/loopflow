use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::lfd::http::dto::{ListResponse, StimulusDefDto, WaveSchemaDto};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, run_store, ApiResult};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct StimulusDef {
    pub kind: String,
    pub cron: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct WaveSchema {
    pub name: String,
    pub flow: String,
    pub area: Vec<String>,
    pub stimulus: Option<StimulusDef>,
    pub direction: Option<Vec<String>>,
    pub owner: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaveSchemaSource {
    Builtin,
    Local,
}

impl WaveSchemaSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedWaveSchema {
    pub(crate) schema_ref: String,
    pub(crate) source: WaveSchemaSource,
    pub(crate) schema: WaveSchema,
}

#[derive(Debug)]
pub(crate) enum SchemaDiscoveryError {
    DuplicateLocalNames {
        names: Vec<String>,
        files: Vec<String>,
    },
}

impl SchemaDiscoveryError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::DuplicateLocalNames { names, files } => format!(
                "duplicate local schema names found ({}) in: {}",
                names.join(", "),
                files.join(", ")
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) enum SchemaResolveError {
    NotFound {
        input: String,
    },
    Ambiguous {
        input: String,
        candidates: Vec<String>,
    },
    Discovery(SchemaDiscoveryError),
}

impl SchemaResolveError {
    pub(crate) fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Ambiguous { .. } | Self::Discovery(_) => StatusCode::CONFLICT,
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            Self::NotFound { input } => format!("wave schema '{input}' not found"),
            Self::Ambiguous { input, candidates } => format!(
                "wave schema '{input}' is ambiguous; candidates: {}",
                candidates.join(", ")
            ),
            Self::Discovery(err) => err.message(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListWaveSchemasQuery {
    repo: Option<String>,
}

pub async fn list_wave_schemas_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWaveSchemasQuery>,
) -> ApiResult<ListResponse<WaveSchemaDto>> {
    let repo = query.repo.unwrap_or_else(|| ".".to_string());
    let repo_path = PathBuf::from(&repo);

    let schemas = discover_wave_schemas(&repo_path)
        .map_err(|err| api_error(StatusCode::CONFLICT, err.message()))?;

    let waves = run_store(&state.store, {
        let repo = repo.clone();
        move |store| store.list_waves(Some(&repo))
    })
    .await
    .map_err(map_store_error)?;

    let mut active_by_ref: HashMap<String, String> = HashMap::new();
    let mut local_by_name: HashMap<String, &ResolvedWaveSchema> = HashMap::new();
    let mut builtin_by_name: HashMap<String, &ResolvedWaveSchema> = HashMap::new();

    for schema in &schemas {
        match schema.source {
            WaveSchemaSource::Local => {
                local_by_name.insert(schema.schema.name.clone(), schema);
            }
            WaveSchemaSource::Builtin => {
                builtin_by_name.insert(schema.schema.name.clone(), schema);
            }
        }
    }

    for wave in &waves {
        if let Some(schema_ref) = &wave.schema_ref {
            active_by_ref
                .entry(schema_ref.clone())
                .or_insert_with(|| wave.id.to_string());
        }
    }

    for wave in waves.iter().filter(|wave| wave.schema_ref.is_none()) {
        let fallback_name = wave.schema_name.as_deref().unwrap_or(&wave.name);
        let maybe_schema = local_by_name
            .get(fallback_name)
            .or_else(|| builtin_by_name.get(fallback_name));
        if let Some(schema) = maybe_schema {
            active_by_ref
                .entry(schema.schema_ref.clone())
                .or_insert_with(|| wave.id.to_string());
        }
    }

    let data = schemas
        .into_iter()
        .map(|resolved| {
            let WaveSchema {
                name,
                flow,
                area,
                stimulus,
                direction,
                owner,
                description,
            } = resolved.schema;
            WaveSchemaDto {
                name,
                schema_ref: resolved.schema_ref.clone(),
                flow,
                area,
                stimulus: stimulus.map(|stimulus| StimulusDefDto {
                    kind: stimulus.kind,
                    cron: stimulus.cron,
                }),
                direction: direction.unwrap_or_default(),
                owner,
                description,
                source: resolved.source.as_str().to_string(),
                active_wave_id: active_by_ref.get(&resolved.schema_ref).cloned(),
            }
        })
        .collect();

    Ok(Json(ListResponse::new(data, false)))
}

pub(crate) fn discover_wave_schemas(
    repo: &Path,
) -> Result<Vec<ResolvedWaveSchema>, SchemaDiscoveryError> {
    let mut schemas = Vec::new();
    schemas.extend(load_builtin_wave_schemas());
    schemas.extend(load_local_wave_schemas(repo)?);
    schemas.sort_by(|a, b| {
        a.schema
            .name
            .cmp(&b.schema.name)
            .then_with(|| match (a.source, b.source) {
                (WaveSchemaSource::Local, WaveSchemaSource::Builtin) => std::cmp::Ordering::Less,
                (WaveSchemaSource::Builtin, WaveSchemaSource::Local) => std::cmp::Ordering::Greater,
                _ => a.schema_ref.cmp(&b.schema_ref),
            })
    });
    Ok(schemas)
}

pub(crate) fn resolve_wave_schema(
    repo: &Path,
    input: &str,
) -> Result<ResolvedWaveSchema, SchemaResolveError> {
    let schemas = discover_wave_schemas(repo).map_err(SchemaResolveError::Discovery)?;
    let candidates: Vec<ResolvedWaveSchema> = if let Some(name) = input.strip_prefix("builtin://") {
        schemas
            .into_iter()
            .filter(|schema| {
                schema.source == WaveSchemaSource::Builtin && schema.schema.name == name
            })
            .collect()
    } else if input.starts_with("file://") {
        schemas
            .into_iter()
            .filter(|schema| schema.schema_ref == input)
            .collect()
    } else {
        let local: Vec<_> = schemas
            .iter()
            .filter(|schema| {
                schema.source == WaveSchemaSource::Local && schema.schema.name == input
            })
            .cloned()
            .collect();
        if !local.is_empty() {
            local
        } else {
            schemas
                .into_iter()
                .filter(|schema| {
                    schema.source == WaveSchemaSource::Builtin && schema.schema.name == input
                })
                .collect()
        }
    };

    match candidates.as_slice() {
        [] => Err(SchemaResolveError::NotFound {
            input: input.to_string(),
        }),
        [schema] => Ok(schema.clone()),
        _ => Err(SchemaResolveError::Ambiguous {
            input: input.to_string(),
            candidates: candidates
                .iter()
                .map(|schema| schema.schema_ref.clone())
                .collect(),
        }),
    }
}

fn load_builtin_wave_schemas() -> Vec<ResolvedWaveSchema> {
    let mut schemas = Vec::new();
    let mut names: Vec<_> = crate::engine::builtins::builtin_wave_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    names.sort();

    for name in names {
        let Some(raw) = crate::engine::builtins::get_builtin_wave(&name) else {
            continue;
        };
        match serde_yaml::from_str::<WaveSchema>(raw) {
            Ok(schema) => schemas.push(ResolvedWaveSchema {
                schema_ref: format!("builtin://{}", schema.name),
                source: WaveSchemaSource::Builtin,
                schema,
            }),
            Err(err) => warn!(schema = %name, error = %err, "invalid builtin wave schema"),
        }
    }

    schemas
}

fn load_local_wave_schemas(repo: &Path) -> Result<Vec<ResolvedWaveSchema>, SchemaDiscoveryError> {
    let mut schemas = Vec::new();
    let mut schema_files_by_name: HashMap<String, HashSet<String>> = HashMap::new();
    let waves_dir = repo.join("wave");
    let Ok(entries) = std::fs::read_dir(&waves_dir) else {
        return Ok(schemas);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(dir_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let schema_path = path.join(format!("{dir_name}.yaml"));
        if !schema_path.is_file() {
            continue;
        }

        let canonical_schema_path = match schema_path.canonicalize() {
            Ok(path) => path,
            Err(err) => {
                warn!(path = %schema_path.display(), error = %err, "failed to canonicalize wave schema path");
                continue;
            }
        };

        let raw = match std::fs::read_to_string(&canonical_schema_path) {
            Ok(content) => content,
            Err(err) => {
                warn!(path = %canonical_schema_path.display(), error = %err, "failed to read local wave schema");
                continue;
            }
        };

        let schema = match serde_yaml::from_str::<WaveSchema>(&raw) {
            Ok(schema) => schema,
            Err(err) => {
                warn!(path = %canonical_schema_path.display(), error = %err, "invalid local wave schema");
                continue;
            }
        };

        let canonical_display = canonical_schema_path.to_string_lossy().replace('\\', "/");
        schema_files_by_name
            .entry(schema.name.clone())
            .or_default()
            .insert(canonical_display.clone());

        schemas.push(ResolvedWaveSchema {
            schema_ref: format!("file://{canonical_display}"),
            source: WaveSchemaSource::Local,
            schema,
        });
    }

    let mut duplicate_names = Vec::new();
    let mut duplicate_files = Vec::new();
    for (name, files) in schema_files_by_name {
        if files.len() > 1 {
            duplicate_names.push(name);
            duplicate_files.extend(files.into_iter());
        }
    }

    if !duplicate_names.is_empty() {
        duplicate_names.sort();
        duplicate_files.sort();
        return Err(SchemaDiscoveryError::DuplicateLocalNames {
            names: duplicate_names,
            files: duplicate_files,
        });
    }

    Ok(schemas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discover_wave_schemas_includes_builtin_and_local() {
        let temp = tempdir().expect("temp dir");
        let local_dir = temp.path().join("wave").join("local-scan");
        fs::create_dir_all(&local_dir).expect("create local dir");
        fs::write(
            local_dir.join("local-scan.yaml"),
            "name: local-scan\nflow: ship\narea: ['.']\n",
        )
        .expect("write schema");

        let schemas = discover_wave_schemas(temp.path()).expect("discover schemas");
        assert!(schemas
            .iter()
            .any(|schema| schema.schema_ref == "builtin://scan"));
        assert!(schemas
            .iter()
            .any(|schema| schema.schema_ref.starts_with("file://")));
    }

    #[test]
    fn resolve_prefers_local_for_unqualified_names() {
        let temp = tempdir().expect("temp dir");
        let local_dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&local_dir).expect("create local dir");
        fs::write(
            local_dir.join("scan.yaml"),
            "name: scan\nflow: ship\narea: ['.']\n",
        )
        .expect("write schema");

        let schema = resolve_wave_schema(temp.path(), "scan").expect("resolve schema");
        assert_eq!(schema.source, WaveSchemaSource::Local);
        assert!(schema.schema_ref.starts_with("file://"));
    }

    #[test]
    fn resolve_supports_explicit_builtin_reference() {
        let temp = tempdir().expect("temp dir");
        let local_dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&local_dir).expect("create local dir");
        fs::write(
            local_dir.join("scan.yaml"),
            "name: scan\nflow: ship\narea: ['.']\n",
        )
        .expect("write schema");

        let schema =
            resolve_wave_schema(temp.path(), "builtin://scan").expect("resolve builtin schema");
        assert_eq!(schema.source, WaveSchemaSource::Builtin);
        assert_eq!(schema.schema_ref, "builtin://scan");
    }

    #[test]
    fn resolve_unknown_schema_returns_not_found() {
        let temp = tempdir().expect("temp dir");
        let result = resolve_wave_schema(temp.path(), "does-not-exist");
        assert!(matches!(result, Err(SchemaResolveError::NotFound { .. })));
    }

    #[test]
    fn discover_duplicate_local_names_returns_conflict() {
        let temp = tempdir().expect("temp dir");
        let first_dir = temp.path().join("wave").join("alpha");
        let second_dir = temp.path().join("wave").join("beta");
        fs::create_dir_all(&first_dir).expect("create first dir");
        fs::create_dir_all(&second_dir).expect("create second dir");
        fs::write(
            first_dir.join("alpha.yaml"),
            "name: duplicate\nflow: ship\narea: ['.']\n",
        )
        .expect("write first schema");
        fs::write(
            second_dir.join("beta.yaml"),
            "name: duplicate\nflow: scan\narea: ['.']\n",
        )
        .expect("write second schema");

        let result = discover_wave_schemas(temp.path());
        assert!(matches!(
            result,
            Err(SchemaDiscoveryError::DuplicateLocalNames { .. })
        ));
    }
}
