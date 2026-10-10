/// Derived fallback/export spelling, never a definition's identity.
pub fn portable_name(name: &str) -> String {
    name.replace('/', "-")
}

/// Select a literal name before considering portable fallbacks.
pub fn resolve_name<'a>(
    name: &str,
    names: impl IntoIterator<Item = &'a str>,
) -> Result<Option<&'a str>, Vec<&'a str>> {
    let key = portable_name(name);
    let mut candidates = Vec::new();
    for candidate in names {
        if candidate == name {
            return Ok(Some(candidate));
        }
        if portable_name(candidate) == key {
            candidates.push(candidate);
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    match candidates.as_slice() {
        [] => Ok(None),
        [candidate] => Ok(Some(*candidate)),
        _ => Err(candidates),
    }
}
