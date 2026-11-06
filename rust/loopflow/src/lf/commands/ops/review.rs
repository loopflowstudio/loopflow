//! The text form of `lf pr reviews`. Reads and requests go through
//! `engine::pr`; inline threads stay on GitHub.

use serde_json::Value;

/// Render the provider's review report the way `lf pr reviews` prints it:
/// the review commit is kept so an old approval never reads as one of the
/// current head.
pub(super) fn render(reviews: &Value) -> String {
    let text = |value: &Value| match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    let mut out = format!(
        "PR #{}  {}\nHead: {}\n",
        text(&reviews["number"]),
        text(&reviews["url"]),
        text(&reviews["headRefOid"])
    );
    if reviews["isDraft"].as_bool().unwrap_or(false) {
        out.push_str("Draft\n");
    }
    let decision = text(&reviews["reviewDecision"]);
    out.push_str(&format!(
        "Review decision: {}\n",
        if decision.is_empty() {
            "not reported"
        } else {
            &decision
        }
    ));
    out.push_str("Requested reviewers:");
    let requested = reviews["reviewRequests"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if requested.is_empty() {
        out.push_str(" none");
    }
    for request in &requested {
        let name = if request["login"].is_null() {
            &request["name"]
        } else {
            &request["login"]
        };
        out.push(' ');
        out.push_str(&text(name));
    }
    out.push_str("\nReview submissions (inline discussion on GitHub):\n");
    let submissions = reviews["reviews"].as_array().cloned().unwrap_or_default();
    if submissions.is_empty() {
        out.push_str("  none\n");
    }
    for review in &submissions {
        out.push_str(&format!(
            "  {}: {}",
            text(&review["author"]["login"]),
            text(&review["state"])
        ));
        if !review["commit"].is_null() {
            out.push_str(&format!(" at {}", text(&review["commit"]["oid"])));
        }
        out.push('\n');
        let body = text(&review["body"]);
        if !body.is_empty() {
            out.push_str(&body);
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
        let reviews = serde_json::json!({
            "number": 42, "url": "https://example.com/pull/42", "headRefOid": "new-head",
            "isDraft": false, "reviewDecision": null,
            "reviewRequests": [{"login": "alice"}, {"name": "platform"}],
            "reviews": [{"author": {"login": "maya"}, "state": "APPROVED", "body": "",
                         "commit": {"oid": "older-head"}}]
        });
        assert_eq!(
            render(&reviews),
            "PR #42  https://example.com/pull/42\nHead: new-head\nReview decision: not reported\n\
             Requested reviewers: alice platform\nReview submissions (inline discussion on GitHub):\n\
             \x20 maya: APPROVED at older-head\n"
        );
    }
}
