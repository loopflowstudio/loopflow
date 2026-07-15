use std::path::Path;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildProvenance {
    Development,
    Release,
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

pub fn source_root() -> &'static Path {
    Path::new(env!("LOOPFLOW_BUILD_SOURCE_ROOT"))
}

pub fn source_identity() -> String {
    source_identity_for(source_root())
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

    use super::{parse_provenance, provenance, source_identity_for, source_root, BuildProvenance};

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
        assert!(source_root().is_absolute());
        assert_eq!(
            parse_provenance("development"),
            Some(BuildProvenance::Development)
        );
        assert_eq!(parse_provenance("release"), Some(BuildProvenance::Release));
        assert_eq!(parse_provenance("other"), None);
    }
}
