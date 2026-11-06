//! The text form of `lf pr reviews`. Reads and requests go through
//! `engine::pr`; inline threads stay on GitHub.

use crate::engine::pr::Reviews;

/// Render the review report the way `lf pr reviews` prints it: the review
/// commit is kept so an old approval never reads as one of the current head.
pub(super) fn render(reviews: &Reviews) -> String {
    let mut out = format!(
        "PR #{}  {}\nHead: {}\n",
        reviews.number,
        reviews.url,
        reviews.head.as_deref().unwrap_or_default()
    );
    if reviews.draft {
        out.push_str("Draft\n");
    }
    let decision = reviews.decision.as_deref().unwrap_or_default();
    out.push_str(&format!(
        "Review decision: {}\n",
        if decision.is_empty() {
            "not reported"
        } else {
            decision
        }
    ));
    out.push_str("Requested reviewers:");
    if reviews.requested.is_empty() {
        out.push_str(" none");
    }
    for request in &reviews.requested {
        out.push(' ');
        out.push_str(
            request
                .login
                .as_deref()
                .or(request.name.as_deref())
                .unwrap_or_default(),
        );
    }
    out.push_str("\nReview submissions (inline discussion on GitHub):\n");
    if reviews.reviews.is_empty() {
        out.push_str("  none\n");
    }
    for review in &reviews.reviews {
        let author = review.author.as_ref().map_or("", |author| &author.login);
        out.push_str(&format!("  {author}: {}", review.state));
        if let Some(commit) = &review.commit {
            out.push_str(&format!(" at {}", commit.oid));
        }
        out.push('\n');
        if !review.body.is_empty() {
            out.push_str(&review.body);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn renders_the_review_report_with_its_commits() {
        let reviews = serde_json::from_value(serde_json::json!({
            "number": 42, "url": "https://example.com/pull/42", "headRefOid": "new-head",
            "isDraft": false, "reviewDecision": "",
            "reviewRequests": [{"__typename": "User", "login": "alice"},
                               {"__typename": "Team", "name": "platform", "slug": "platform"}],
            "reviews": [{"id": "r1", "author": {"login": "maya"}, "state": "APPROVED", "body": "",
                         "submittedAt": "2026-11-05T00:00:00Z", "commit": {"oid": "older-head"}}]
        }))
        .unwrap();
        assert_eq!(
            render(&reviews),
            "PR #42  https://example.com/pull/42\nHead: new-head\nReview decision: not reported\n\
             Requested reviewers: alice platform\nReview submissions (inline discussion on GitHub):\n\
             \x20 maya: APPROVED at older-head\n"
        );
    }
}
