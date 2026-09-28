use crate::engine::platform::open_url_checked;
use crate::ops::error::{OpsError, OpsResult};

/// Where a pull request is opened for review. The only surface today is
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

/// Present an existing draft or ready PR. Only `lf pr open` calls this boundary;
/// presentation does not change readiness. A failed browser launch leaves the
/// PR available at its URL.
pub fn present_pr_review(url: &str) -> OpsResult<()> {
    match resolve_review_surface() {
        ReviewSurface::GithubBrowser => open_url_checked(url).map_err(|err| {
            OpsError::Message(format!("failed to open review surface for {url}: {err}"))
        }),
    }
}
