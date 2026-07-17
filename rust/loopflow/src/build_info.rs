use std::path::Path;

use sha2::{Digest, Sha256};

use crate::engine::git;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedCommit {
    pub revision: String,
    pub subject: String,
}

/// Where the running binary's revision sits relative to merged upstream work.
///
/// `Behind` is the only state that means a merged fix is missing. An absent
/// object or failed comparison is `Unprovable`, never `Current` or `OffMain`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildFreshness {
    Current {
        revision: String,
    },
    Behind {
        revision: String,
        missing: Vec<MergedCommit>,
    },
    OffMain {
        revision: String,
    },
    Unprovable {
        reason: String,
    },
}

/// Compare a build revision against `upstream` (e.g. `origin/main`) in `repo`.
pub fn classify_revision(revision: &str, repo: &Path, upstream: &str) -> BuildFreshness {
    let unprovable = |reason: String| BuildFreshness::Unprovable { reason };

    if !is_revision(revision) {
        return unprovable(format!("build revision {revision} is not a commit id"));
    }

    match git::commit_exists(repo, revision) {
        Ok(true) => {}
        Ok(false) => {
            return unprovable(format!(
                "build revision {} is not a commit in {}, so this checkout cannot say what the binary is missing",
                short_revision(revision),
                repo.display()
            ))
        }
        Err(error) => {
            return unprovable(format!("git could not read {}: {error}", repo.display()))
        }
    }

    let tip = match git::rev_parse(repo, upstream) {
        Ok(tip) => tip.trim().to_string(),
        Err(error) => return unprovable(format!("git could not resolve {upstream}: {error}")),
    };
    if tip == revision {
        return BuildFreshness::Current {
            revision: revision.to_string(),
        };
    }

    // The object is established above, so a `false` here is a real answer about
    // ancestry rather than a swallowed git failure.
    match git::is_ancestor(repo, revision, upstream) {
        Ok(true) => match git::commits_between(repo, revision, upstream) {
            Ok(commits) => BuildFreshness::Behind {
                revision: revision.to_string(),
                missing: commits
                    .into_iter()
                    .map(|(revision, subject)| MergedCommit { revision, subject })
                    .collect(),
            },
            Err(error) => unprovable(format!(
                "git could not list {revision}..{upstream}: {error}"
            )),
        },
        Ok(false) => BuildFreshness::OffMain {
            revision: revision.to_string(),
        },
        Err(error) => unprovable(format!("git could not compare against {upstream}: {error}")),
    }
}

pub fn short_revision(revision: &str) -> &str {
    revision.get(..9).unwrap_or(revision)
}

fn is_revision(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildProvenance {
    Development,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
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
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        classify_revision, migration_authority, parse_provenance, provenance, source_identity_for,
        source_revision, source_root, BuildFreshness, BuildProvenance, MigrationAuthority,
    };

    /// A throwaway `A -> B -> C` repo keeps classifier tests offline.
    struct Fixture {
        _directory: tempfile::TempDir,
        repo: PathBuf,
        commits: Vec<String>,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let repo = directory.path().to_path_buf();
            git(&repo, &["init", "--initial-branch=main"]);
            git(&repo, &["config", "user.email", "test@loopflow.studio"]);
            git(&repo, &["config", "user.name", "Test"]);
            let mut commits = Vec::new();
            for subject in ["A: first", "B: second", "C: third"] {
                std::fs::write(repo.join("file"), subject).unwrap();
                git(&repo, &["add", "file"]);
                git(&repo, &["commit", "-m", subject]);
                commits.push(git(&repo, &["rev-parse", "HEAD"]).trim().to_string());
            }
            Self {
                _directory: directory,
                repo,
                commits,
            }
        }
    }

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn a_build_behind_the_tip_names_every_merged_commit_it_lacks() {
        let fixture = Fixture::new();

        let freshness = classify_revision(&fixture.commits[0], &fixture.repo, "main");

        let BuildFreshness::Behind {
            revision, missing, ..
        } = freshness
        else {
            panic!("a build at A is behind main: {freshness:?}");
        };
        assert_eq!(revision, fixture.commits[0]);
        let subjects: Vec<&str> = missing
            .iter()
            .map(|commit| commit.subject.as_str())
            .collect();
        assert_eq!(subjects, ["B: second", "C: third"], "oldest first");
        assert_eq!(missing[0].revision, fixture.commits[1]);
    }

    #[test]
    fn a_build_at_the_tip_is_current() {
        let fixture = Fixture::new();

        let freshness = classify_revision(&fixture.commits[2], &fixture.repo, "main");

        assert_eq!(
            freshness,
            BuildFreshness::Current {
                revision: fixture.commits[2].clone()
            }
        );
    }

    #[test]
    fn absent_objects_and_failed_comparisons_are_unprovable() {
        let fixture = Fixture::new();
        let stranger = "0123456789abcdef0123456789abcdef01234567";

        assert!(matches!(
            classify_revision(stranger, &fixture.repo, "main"),
            BuildFreshness::Unprovable { .. }
        ));
        assert!(matches!(
            classify_revision(&fixture.commits[0], &fixture.repo, "missing"),
            BuildFreshness::Unprovable { .. }
        ));
    }

    #[test]
    fn a_build_off_the_upstream_line_is_not_behind() {
        let fixture = Fixture::new();
        git(
            &fixture.repo,
            &["checkout", "-b", "feature", &fixture.commits[1]],
        );
        std::fs::write(fixture.repo.join("file"), "feature work").unwrap();
        git(&fixture.repo, &["add", "file"]);
        git(&fixture.repo, &["commit", "-m", "feature: work"]);
        let head = git(&fixture.repo, &["rev-parse", "HEAD"])
            .trim()
            .to_string();

        let freshness = classify_revision(&head, &fixture.repo, "main");

        assert_eq!(freshness, BuildFreshness::OffMain { revision: head });
    }

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
