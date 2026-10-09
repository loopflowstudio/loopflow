/// Portable comparison and display key; source paths and native names stay unchanged.
pub fn definition_key(name: &str) -> String {
    name.replace('/', "-")
}
