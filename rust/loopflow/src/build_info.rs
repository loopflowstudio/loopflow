//! Values embedded by the application build script, supplied to execution at entry.
use std::path::Path;

pub use loopflow_execution::build_identity::{
    classify_revision, label, parse_provenance, provenance, short_revision, source_identity,
    source_revision, source_root, BuildFreshness, BuildProvenance, MergedCommit,
};

pub const BUILD_VERSION: &str = env!("LOOPFLOW_BUILD_VERSION");
pub const VERSION_LINE: &str = env!("LOOPFLOW_VERSION_LINE");

pub fn initialize() {
    let root = env!("LOOPFLOW_BUILD_SOURCE_ROOT");
    loopflow_execution::build_identity::initialize(
        loopflow_execution::build_identity::BuildIdentity {
            provenance: parse_provenance(env!("LOOPFLOW_COMPILED_PROVENANCE"))
                .expect("build script embeds validated provenance"),
            source_root: (root != "release").then(|| Path::new(root)),
            source_revision: env!("LOOPFLOW_BUILD_SOURCE_REVISION"),
            version: BUILD_VERSION,
            label: Some(env!("LOOPFLOW_BUILD_LABEL")).filter(|label| !label.is_empty()),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{
        label, parse_provenance, provenance, short_revision, source_identity, source_revision,
        source_root, BuildProvenance, BUILD_VERSION, VERSION_LINE,
    };
    use std::path::Path;
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
                assert_eq!(source_identity(), "release");
            }
        }
        assert_eq!(
            parse_provenance("development"),
            Some(BuildProvenance::Development)
        );
        assert_eq!(parse_provenance("release"), Some(BuildProvenance::Release));
        assert_eq!(parse_provenance("other"), None);
        assert!(!source_revision().is_empty());
        let package_version = env!("CARGO_PKG_VERSION");
        assert!(
            BUILD_VERSION == package_version
                || BUILD_VERSION.starts_with(&format!("{package_version}+"))
        );
        if source_revision() != "unknown" && BUILD_VERSION != package_version {
            assert!(BUILD_VERSION.contains(short_revision(source_revision())));
            let label = label().expect("an unreleased build is labeled");
            let kind = if loopflow_execution::store::migrations::unmerged_migrations().is_empty() {
                "build"
            } else {
                "schema work"
            };
            assert!(
                label
                    .trim_start_matches("dirty ")
                    .starts_with(&format!("{kind} {}", short_revision(source_revision()))),
                "{label}"
            );
            assert_eq!(VERSION_LINE, format!("{BUILD_VERSION} ({label})"));
        } else {
            assert_eq!(label(), None);
            assert_eq!(VERSION_LINE, BUILD_VERSION);
        }
    }
}
