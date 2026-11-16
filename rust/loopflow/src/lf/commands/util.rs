use std::path::PathBuf;

use anyhow::Result;

pub fn find_repo_root() -> Result<PathBuf> {
    crate::repo::find_repo_root()
}
