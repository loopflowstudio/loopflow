use std::path::Path;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildProvenance {
    Development,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationAuthority {
    Published,
    ValidationOnly,
}

impl BuildProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Release => "release",
        }
    }

    pub fn is_release(self) -> bool {
        self == Self::Release
    }
}

impl std::fmt::Display for BuildProvenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn provenance() -> BuildProvenance {
    parse_provenance(env!("LOOPFLOW_BUILD_PROVENANCE"))
        .expect("build script embeds a validated Loopflow provenance")
}

pub fn source_root() -> Option<&'static Path> {
    let root = env!("LOOPFLOW_BUILD_SOURCE_ROOT");
    (root != "release").then(|| Path::new(root))
}

pub fn source_identity() -> String {
    source_root()
        .map(source_identity_for)
        .unwrap_or_else(|| "release".to_string())
}

pub fn source_revision() -> &'static str {
    env!("LOOPFLOW_BUILD_SOURCE_REVISION")
}

pub fn migration_authority() -> MigrationAuthority {
    match env!("LOOPFLOW_MIGRATION_AUTHORITY") {
        "published" => MigrationAuthority::Published,
        "validation_only" => MigrationAuthority::ValidationOnly,
        _ => unreachable!("build script validates migration authority"),
    }
}

fn parse_provenance(value: &str) -> Option<BuildProvenance> {
    match value {
        "development" => Some(BuildProvenance::Development),
        "release" => Some(BuildProvenance::Release),
        _ => None,
    }
}

fn source_identity_for(root: &Path) -> String {
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("loopflow")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let digest = Sha256::digest(root.as_os_str().as_encoded_bytes());
    format!("{name}-{}", hex::encode(&digest[..6]))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        migration_authority, parse_provenance, provenance, source_identity_for, source_revision,
        source_root, BuildProvenance, MigrationAuthority,
    };

    #[test]
    fn source_identity_is_stable_and_distinguishes_worktrees() {
        assert_eq!(
            source_identity_for(Path::new("/src/loopflow")),
            source_identity_for(Path::new("/src/loopflow"))
        );
        assert_ne!(
            source_identity_for(Path::new("/src/loopflow")),
            source_identity_for(Path::new("/src/loopflow-feature"))
        );
    }

    #[test]
    fn embedded_build_metadata_is_valid_in_checkouts_and_packaged_sources() {
        assert!(matches!(
            provenance(),
            BuildProvenance::Development | BuildProvenance::Release
        ));
        match provenance() {
            BuildProvenance::Development => {
                assert!(source_root().is_some_and(Path::is_absolute));
            }
            BuildProvenance::Release => {
                assert_eq!(source_root(), None);
                assert_eq!(super::source_identity(), "release");
            }
        }
        assert_eq!(
            parse_provenance("development"),
            Some(BuildProvenance::Development)
        );
        assert_eq!(parse_provenance("release"), Some(BuildProvenance::Release));
        assert_eq!(parse_provenance("other"), None);
        assert!(matches!(
            migration_authority(),
            MigrationAuthority::Published | MigrationAuthority::ValidationOnly
        ));
        assert!(!source_revision().is_empty());
    }
}
