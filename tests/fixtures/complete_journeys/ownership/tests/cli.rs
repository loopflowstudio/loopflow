use std::process::Command;

use journey_ownership::definitions::skill;

#[test]
fn cli_and_library_resolve_the_same_skill() {
    let output = Command::new(env!("CARGO_BIN_EXE_journey-ownership"))
        .arg("implement")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        skill("implement").unwrap()
    );
}

#[test]
fn missing_skill_is_a_caller_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_journey-ownership"))
        .arg("missing")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(skill("missing"), None);
}
