//! A change as an agent is shown it: every changed file, the command that
//! reads one file's patch, and the whole patch when it is small.

use std::collections::HashSet;
use std::path::Path;

use crate::error::GitError;
use crate::git::git_stdout;
use crate::output::format_int;
use crate::prompt::count_tokens;

/// Under this many tokens a patch goes into the prompt whole; over it, none
/// of it does and the agent reads the files it picks.
pub const PATCH_INLINE_TOKENS: usize = 15_000;

/// A patch this large cannot be under [`PATCH_INLINE_TOKENS`]; skip counting.
const PATCH_COUNTED_BYTES: usize = 200_000;

/// Which two trees a change compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeRange {
    /// The index against HEAD: what the next commit holds.
    Staged,
    /// HEAD against its merge base with this ref: what a branch adds.
    Since(String),
    /// The index and worktree against HEAD.
    Uncommitted,
}

impl ChangeRange {
    fn git_arg(&self) -> String {
        match self {
            Self::Staged => "--cached".to_owned(),
            Self::Since(base) => format!("{base}...HEAD"),
            Self::Uncommitted => "HEAD".to_owned(),
        }
    }

    fn title(&self) -> String {
        match self {
            Self::Staged => "Staged".to_owned(),
            Self::Since(base) => format!("Changed since {base}"),
            Self::Uncommitted => "Uncommitted".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub renamed_from: Option<String>,
    /// Added and deleted lines; `None` for a binary file.
    pub lines: Option<(u64, u64)>,
    /// A note added under `scratch/` on this branch, cleared when it lands.
    pub working_note: bool,
    /// Not in Git yet, so it has no patch: the file itself is the change.
    pub untracked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub range: ChangeRange,
    pub files: Vec<ChangedFile>,
    /// The whole patch, when it is under [`PATCH_INLINE_TOKENS`].
    pub patch: Option<String>,
    pub patch_bytes: usize,
}

impl Change {
    /// # Errors
    ///
    /// Fails when Git cannot read the range, such as a base ref that does
    /// not exist.
    pub fn read(repo: &Path, range: ChangeRange) -> Result<Self, GitError> {
        let range_arg = range.git_arg();
        let numstat = git_stdout(repo, &["diff", "--numstat", "-z", &range_arg])?;
        let mut files = parse_numstat(&numstat);
        match &range {
            ChangeRange::Staged => {}
            ChangeRange::Since(base) => {
                let merge_base = crate::git::merge_base(repo, "HEAD", base)?;
                let notes: HashSet<String> = crate::git::scratch::added(repo, &merge_base, "HEAD")?
                    .into_iter()
                    .collect();
                for file in &mut files {
                    file.working_note = notes.contains(&file.path);
                }
            }
            ChangeRange::Uncommitted => files.extend(untracked_files(repo)?),
        }
        let patch = git_stdout(repo, &["diff", &range_arg])?;
        let patch_bytes = patch.len();
        let inline =
            patch_bytes <= PATCH_COUNTED_BYTES && count_tokens(&patch) < PATCH_INLINE_TOKENS;
        Ok(Self {
            range,
            files,
            patch: inline.then_some(patch),
            patch_bytes,
        })
    }

    /// The one prompt section that shows this change to an agent.
    pub fn prompt_section(&self) -> String {
        let count = self.files.len();
        let range = self.range.git_arg();
        let mut section = format!(
            "{}, {count} file{}. Read one file's patch with:\n  git diff {range} -- '<path>'\n",
            self.range.title(),
            if count == 1 { "" } else { "s" },
        );
        // Git pairs a rename only when both paths are in the pathspec; with
        // the new path alone it returns the whole file as added.
        if self.files.iter().any(|file| file.renamed_from.is_some()) {
            section.push_str(&format!(
                "A renamed file needs both paths:\n  git diff {range} -- '<path>' '<old path>'\n"
            ));
        }
        section.push('\n');

        let added_width = self.count_width(|(added, _)| added);
        let deleted_width = self.count_width(|(_, deleted)| deleted);
        for file in &self.files {
            let counts = match file.lines {
                Some((added, deleted)) => format!(
                    "{:>added_width$}  {:<deleted_width$}",
                    format!("+{added}"),
                    format!("-{deleted}"),
                ),
                None => format!(
                    "{:<width$}",
                    "binary",
                    width = added_width + 2 + deleted_width
                ),
            };
            section.push_str(&format!("  {counts}   {}", file.path));
            if let Some(old) = &file.renamed_from {
                section.push_str(&format!(" (from {old})"));
            }
            if file.untracked {
                section.push_str(" (untracked, no patch: read the file)");
            }
            if file.working_note {
                section.push_str(" (working note, cleared when this lands)");
            }
            section.push('\n');
        }

        match &self.patch {
            // Only untracked files changed: there is no patch to show.
            Some(patch) if patch.is_empty() => {}
            Some(patch) => {
                let fence = fence_for(patch);
                section.push_str(&format!(
                    "\nThe whole patch:\n{fence}diff\n{}\n{fence}\n",
                    patch.trim_end()
                ));
            }
            None => section.push_str(&format!(
                "\nThe patch is {} bytes, too large to include. Start with the files that\n\
                 carry the change.\n",
                format_int(self.patch_bytes as u64)
            )),
        }
        section
    }

    fn count_width(&self, side: impl Fn((u64, u64)) -> u64) -> usize {
        self.files
            .iter()
            .filter_map(|file| file.lines)
            .map(|lines| side(lines).to_string().len() + 1)
            .max()
            .unwrap_or(2)
    }
}

/// Files Git does not track or ignore. `git diff` leaves them out, and an
/// uncommitted change that adds a file is not complete without them.
fn untracked_files(repo: &Path) -> Result<Vec<ChangedFile>, GitError> {
    let listing = git_stdout(repo, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    Ok(listing
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(|path| ChangedFile {
            path: path.to_owned(),
            renamed_from: None,
            lines: std::fs::read_to_string(repo.join(path))
                .ok()
                .map(|text| (text.lines().count() as u64, 0)),
            working_note: false,
            untracked: true,
        })
        .collect())
}

/// `--numstat -z` writes `added\tdeleted\tpath\0`, and for a rename an empty
/// path followed by the old and new paths as their own fields.
fn parse_numstat(numstat: &str) -> Vec<ChangedFile> {
    let mut fields = numstat.split('\0');
    let mut files = Vec::new();
    while let Some(record) = fields.next() {
        let mut parts = record.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (path, renamed_from) = if path.is_empty() {
            let old = fields.next().unwrap_or_default();
            let new = fields.next().unwrap_or_default();
            (new, Some(old.to_owned()))
        } else {
            (path, None)
        };
        files.push(ChangedFile {
            path: path.to_owned(),
            renamed_from,
            lines: added.parse().ok().zip(deleted.parse().ok()),
            working_note: false,
            untracked: false,
        });
    }
    files
}

/// A fence longer than any backtick run in `text`, so a patch that edits
/// Markdown cannot close its own block.
fn fence_for(text: &str) -> String {
    let longest = text.split(|ch| ch != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

#[cfg(test)]
mod tests {
    use super::{format_int, Change, ChangeRange};
    use loopflow_test_support::TestRepo;

    fn git(repo: &TestRepo, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {args:?}");
    }

    #[test]
    fn a_large_staged_change_lists_every_file_and_the_read_command_without_a_patch() {
        let repo = TestRepo::new();
        let generated: String = (0..20_000).map(|n| format!("row {n} of data\n")).collect();
        repo.create_file("aaa_generated.txt", &generated);
        repo.create_file("src/zzz_retry.rs", "fn retry() {}\n");
        repo.stage_all();

        let change = Change::read(repo.path(), ChangeRange::Staged).unwrap();
        let section = change.prompt_section();

        assert!(section.starts_with("Staged, 2 files. Read one file's patch with:\n"));
        assert!(section.contains("  git diff --cached -- '<path>'\n\n"));
        assert!(!section.contains("both paths"), "{section}");
        assert!(
            section.contains("  +20000  -0   aaa_generated.txt\n"),
            "{section}"
        );
        assert!(
            section.contains("      +1  -0   src/zzz_retry.rs\n"),
            "{section}"
        );
        assert_eq!(change.patch, None);
        assert!(!section.contains("row 1 of data"));
        assert!(
            section.contains(&format!(
                "The patch is {} bytes, too large to include.",
                format_int(change.patch_bytes as u64)
            )),
            "{section}"
        );
    }

    #[test]
    fn a_small_change_carries_its_whole_patch() {
        let repo = TestRepo::new();
        repo.create_file("notes.md", "```rust\nfn retry() {}\n```\n");
        repo.stage_all();

        let section = Change::read(repo.path(), ChangeRange::Staged)
            .unwrap()
            .prompt_section();

        assert!(section.contains("  +3  -0   notes.md\n"), "{section}");
        assert!(
            section.contains("The whole patch:\n````diff\ndiff --git"),
            "{section}"
        );
        assert!(section.contains("+fn retry() {}\n"));
        assert!(section.ends_with("\n````\n"), "{section}");
    }

    #[test]
    fn renames_and_binary_files_print_paths_and_commands_that_read_them() {
        let repo = TestRepo::new();
        let body: String = (0..40).map(|n| format!("line {n}\n")).collect();
        repo.create_file("a deeply/nested directory/old name.rs", &body);
        repo.stage_all();
        repo.commit("add a file to move");
        git(
            &repo,
            &[
                "mv",
                "a deeply/nested directory/old name.rs",
                "a deeply/nested directory/new name.rs",
            ],
        );
        std::fs::write(repo.path().join("logo.png"), [0u8, 159, 146, 150, 0, 1]).unwrap();
        repo.stage_all();

        let change = Change::read(repo.path(), ChangeRange::Staged).unwrap();
        let section = change.prompt_section();

        assert!(
            section.contains(
                "Read one file's patch with:\n  git diff --cached -- '<path>'\n\
                 A renamed file needs both paths:\n  git diff --cached -- '<path>' '<old path>'\n\n"
            ),
            "{section}"
        );
        assert!(
            section.contains(
                "  +0  -0   a deeply/nested directory/new name.rs \
                 (from a deeply/nested directory/old name.rs)\n"
            ),
            "{section}"
        );
        assert!(section.contains("  binary   logo.png\n"), "{section}");
    }

    #[test]
    fn a_branch_change_marks_the_notes_it_added_under_scratch() {
        let repo = TestRepo::new();
        repo.create_branch("feature");
        repo.create_file("scratch/plan.md", "why the retry exists\n");
        repo.create_file("src/retry.rs", "fn retry() {}\n");
        repo.stage_all();
        repo.commit("add a retry");
        repo.create_file("uncommitted.txt", "not part of the branch yet\n");
        repo.stage_all();

        let change =
            Change::read(repo.path(), ChangeRange::Since("origin/main".to_owned())).unwrap();
        let section = change.prompt_section();

        assert!(
            section
                .starts_with("Changed since origin/main, 2 files. Read one file's patch with:\n"),
            "{section}"
        );
        assert!(section.contains("  git diff origin/main...HEAD -- '<path>'\n"));
        assert!(
            section.contains("scratch/plan.md (working note, cleared when this lands)\n"),
            "{section}"
        );
        assert!(section.contains("  +1  -0   src/retry.rs\n"), "{section}");
        assert!(!section.contains("uncommitted.txt"));

        let uncommitted = Change::read(repo.path(), ChangeRange::Uncommitted).unwrap();
        assert_eq!(uncommitted.files.len(), 1);
        assert!(uncommitted
            .prompt_section()
            .contains("  git diff HEAD -- '<path>'\n"));
    }

    #[test]
    fn uncommitted_work_lists_the_files_git_does_not_track_yet() {
        let repo = TestRepo::new();
        repo.create_file("src/retry.rs", "fn retry() {}\n");
        repo.stage_all();
        repo.commit("add a retry");
        repo.create_file("src/retry.rs", "fn retry() { send() }\n");
        repo.create_file(
            "src/backoff.rs",
            "const DELAY_MS: u64 = 250;\nconst TRIES: u8 = 2;\n",
        );

        let section = Change::read(repo.path(), ChangeRange::Uncommitted)
            .unwrap()
            .prompt_section();

        assert!(section.starts_with("Uncommitted, 2 files."), "{section}");
        assert!(section.contains("  +1  -1   src/retry.rs\n"), "{section}");
        assert!(
            section.contains("  +2  -0   src/backoff.rs (untracked, no patch: read the file)\n"),
            "{section}"
        );

        std::fs::write(repo.path().join("src/retry.rs"), "fn retry() {}\n").unwrap();
        let only_untracked = Change::read(repo.path(), ChangeRange::Uncommitted)
            .unwrap()
            .prompt_section();
        assert!(
            only_untracked.starts_with("Uncommitted, 1 file."),
            "{only_untracked}"
        );
        assert!(!only_untracked.contains("patch:\n"), "{only_untracked}");
    }

    #[test]
    fn an_unreadable_base_is_an_error_not_an_empty_change() {
        let repo = TestRepo::new();
        assert!(Change::read(repo.path(), ChangeRange::Since("origin/absent".to_owned())).is_err());
    }
}
