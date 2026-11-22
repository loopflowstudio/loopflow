//! Portable person display values; acquisition stays with configuration and providers.

pub fn normalize_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}
