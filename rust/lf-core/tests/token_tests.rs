use lf_core::count_tokens;

#[test]
fn token_counting() {
    assert_eq!(count_tokens(""), 1);
    assert!(count_tokens("hello") >= 1);
    let long = "a".repeat(30);
    assert!(count_tokens(&long) >= 10);
}
