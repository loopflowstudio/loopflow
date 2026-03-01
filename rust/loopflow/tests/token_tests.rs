use loopflow::engine::count_tokens;

#[test]
fn token_counting() {
    assert_eq!(count_tokens(""), 1);
    assert_eq!(count_tokens("hello"), 1);
    let long = "a".repeat(30);
    assert_eq!(count_tokens(&long), 5);
}
