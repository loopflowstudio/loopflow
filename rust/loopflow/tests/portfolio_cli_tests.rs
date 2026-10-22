use std::path::Path;
use std::process::{Command, Stdio};

use loopflow::portfolio::{Portfolio, PortfolioMode};

fn portfolio(home: &Path, args: &[&str]) -> Portfolio {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env("LF_HOME", home)
        .current_dir(home)
        .args(["portfolio", "--json"])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn portfolio_cli_works_outside_repositories_and_preserves_unavailable_imports() {
    let home = tempfile::tempdir().unwrap();
    assert!(!portfolio(home.path(), &[]).configured);
    let imported = portfolio(home.path(), &["import", "/unavailable/portfolio-repo"]);
    assert_eq!(imported.mode, PortfolioMode::Manual);
    assert!(imported.repos[0].error.is_some());
    assert_eq!(
        portfolio(home.path(), &[]).repos[0].path,
        imported.repos[0].path
    );
    assert!(
        portfolio(home.path(), &["remove", "/unavailable/portfolio-repo"])
            .repos
            .is_empty()
    );
    assert!(portfolio(home.path(), &["folder", "/unavailable/folder"])
        .error
        .is_some());
    assert_eq!(
        portfolio(home.path(), &["manual"]).mode,
        PortfolioMode::Manual
    );
}
