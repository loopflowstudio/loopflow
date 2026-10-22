use std::process::Command;

#[test]
fn cli_volume_records_real_reads_without_contents() {
    let home = tempfile::tempdir().unwrap();
    let receipts = tempfile::tempdir().unwrap();
    for measured in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("LF_") {
                command.env_remove(key);
            }
        }
        command
            .args(["session", "list", "--json", "--all"])
            .current_dir(home.path())
            .env("LF_HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"));
        if measured {
            command.env("LF_PERF_OUTPUT", receipts.path());
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!([])
        );
        let files: Vec<_> = std::fs::read_dir(receipts.path()).unwrap().collect();
        assert_eq!(files.len(), usize::from(measured));
    }
    let file = std::fs::read_dir(receipts.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let text = std::fs::read_to_string(file.path()).unwrap();
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.first().unwrap()["event"], "start");
    let end = events.last().unwrap();
    assert_eq!(end["event"], "end");
    assert!(end["connections"].as_u64().unwrap() > 0);
    assert!(end["statements"].as_u64().unwrap() > 0);
    assert!(end["rows"].as_u64().unwrap() > 0);
    assert!(!text.contains(home.path().to_str().unwrap()));
    assert!(!text.contains("SELECT"));
    assert!(!text.contains("session"));
}
