use crate::engine::platform::open_url_checked;
use crate::ops::error::{OpsError, OpsResult};

/// Where a pull request is opened for human review. The only surface today is
/// the GitHub PR page in the default browser; a terminal diff or file browser
/// would be added here as another arm without touching the publication path or
/// any caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ReviewSurface {
    /// Open the PR URL in the default browser.
    #[default]
    GithubBrowser,
}

/// Resolve the preferred review surface. An absent preference uses the GitHub
/// browser default.
fn resolve_review_surface() -> ReviewSurface {
    ReviewSurface::default()
}

/// Present a published PR for review. This is the single presentation boundary:
/// only `lf pr open` calls it, and only after publication has produced `url`.
/// Publication never routes through here, so a failed surface launch can fail
/// `pr open` alone and never makes a published PR look failed.
pub fn present_pr_review(url: &str) -> OpsResult<()> {
    match resolve_review_surface() {
        ReviewSurface::GithubBrowser => open_url_checked(url).map_err(|err| {
            OpsError::Message(format!("failed to open review surface for {url}: {err}"))
        }),
    }
}
