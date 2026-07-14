//! Flat naming for low-level Git worktrees.
//!
//! One name has two projections: `<name>` for the sibling directory suffix
//! and `<author>/<name>` for the branch. It carries no Wave, Task, worker,
//! timestamp, or ancestry semantics.

use crate::engine::worktrees::WorktreeSegment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeName {
    author: String,
    segment: WorktreeSegment,
}

impl WorktreeName {
    pub fn new(author: impl Into<String>, segment: WorktreeSegment) -> Option<Self> {
        let author = author.into();
        let author = author.trim();
        if author.is_empty() || author.contains('/') {
            return None;
        }
        Some(Self {
            author: author.to_string(),
            segment,
        })
    }

    /// Parse either `<author>/<name>` or the local `<name>` projection.
    pub fn parse(raw: &str, fallback_author: &str) -> Option<Self> {
        let raw = raw.trim();
        let (author, segment) = match raw.split_once('/') {
            Some((author, segment)) if !segment.contains('/') => (author, segment),
            Some(_) => return None,
            None => (fallback_author, raw),
        };
        Self::new(author, WorktreeSegment::parse(segment).ok()?)
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn name(&self) -> &str {
        self.segment.as_str()
    }

    pub fn dir_component(&self) -> &str {
        self.name()
    }

    pub fn branch(&self) -> String {
        format!("{}/{}", self.author, self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::WorktreeName;
    use crate::engine::worktrees::WorktreeSegment;

    #[test]
    fn branch_and_directory_are_flat_projections() {
        let name = WorktreeName::new("jack", WorktreeSegment::parse("fix-auth").unwrap()).unwrap();
        assert_eq!(name.author(), "jack");
        assert_eq!(name.name(), "fix-auth");
        assert_eq!(name.dir_component(), "fix-auth");
        assert_eq!(name.branch(), "jack/fix-auth");
    }

    #[test]
    fn parse_accepts_branch_or_directory_projection() {
        let branch = WorktreeName::parse("jack/fix-auth", "fallback").unwrap();
        let directory = WorktreeName::parse("fix-auth", "jack").unwrap();
        assert_eq!(branch, directory);
    }

    #[test]
    fn parse_rejects_recursive_or_dotted_names() {
        assert!(WorktreeName::parse("jack/bugs/fix-auth", "fallback").is_none());
        assert!(WorktreeName::parse("bugs.fix-auth", "jack").is_none());
        assert!(WorktreeName::parse("", "jack").is_none());
        assert!(WorktreeName::parse("fix-auth", "").is_none());
    }
}
