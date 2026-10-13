//! Review requests and reads stay on the existing provider PR. No local review state.

use std::process::Command;

use anyhow::Result;

use crate::lf::commands::util::find_repo_root;
use crate::process::CommandExit;

pub(super) fn request(pr: Option<&str>, reviewers: &[String]) -> Result<()> {
    let mut command = Command::new("gh");
    command.args(["pr", "edit"]);
    if let Some(pr) = pr {
        command.arg(pr);
    }
    for reviewer in reviewers {
        command.arg("--add-reviewer").arg(reviewer);
    }
    run(command)
}

pub(super) fn show(pr: Option<&str>, json: bool) -> Result<()> {
    let mut command = Command::new("gh");
    command.args(["pr", "view"]);
    if let Some(pr) = pr {
        command.arg(pr);
    }
    // Preserve the provider's review commit instead of interpreting an old
    // approval as an approval of the current head. Inline threads stay on GitHub.
    command.args([
        "--json",
        "number,url,headRefOid,isDraft,reviewDecision,reviewRequests,reviews",
    ]);
    if !json {
        command.args(["--template", REVIEW_TEMPLATE]);
    }
    run(command)
}

const REVIEW_TEMPLATE: &str = r##"{{printf "PR #%v  %s\nHead: %s\n" .number .url .headRefOid}}{{if .isDraft}}Draft
{{end}}Review decision: {{if .reviewDecision}}{{.reviewDecision}}{{else}}not reported{{end}}
Requested reviewers:{{range .reviewRequests}} {{if .login}}{{.login}}{{else}}{{.name}}{{end}}{{else}} none{{end}}
Review submissions (inline discussion on GitHub):
{{range .reviews}}{{printf "  %s: %s" .author.login .state}}{{if .commit}}{{printf " at %s" .commit.oid}}{{end}}{{printf "\n"}}{{if .body}}{{printf "%s\n" .body}}{{end}}{{else}}  none
{{end}}"##;

fn run(mut command: Command) -> Result<()> {
    let status = command.current_dir(find_repo_root()?).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(CommandExit(status.code().unwrap_or(1) as u8).into())
    }
}
