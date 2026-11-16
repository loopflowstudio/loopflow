pub fn skill(name: &str) -> Option<&'static str> {
    match name {
        "implement" => Some("Finish the selected journey."),
        _ => None,
    }
}
