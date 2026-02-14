use std::collections::HashMap;

/// In-memory key-value store for agent working state during a session.
///
/// Each block is a named string. Token counts are approximate (via tiktoken-rs).
#[derive(Debug, Default)]
pub struct ContextStore {
    blocks: HashMap<String, String>,
}

impl ContextStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(&self, name: &str) -> Option<&str> {
        self.blocks.get(name).map(|s| s.as_str())
    }

    pub fn write(&mut self, name: String, content: String) {
        self.blocks.insert(name, content);
    }

    pub fn delete(&mut self, name: &str) -> bool {
        self.blocks.remove(name).is_some()
    }

    /// List all blocks with their approximate token counts.
    pub fn list(&self) -> Vec<(String, usize)> {
        let mut entries: Vec<(String, usize)> = self
            .blocks
            .iter()
            .map(|(name, content)| (name.clone(), estimate_tokens(content)))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }
}

/// Approximate token count using tiktoken-rs (cl100k_base).
fn estimate_tokens(text: &str) -> usize {
    tiktoken_rs::cl100k_base()
        .map(|bpe| bpe.encode_with_special_tokens(text).len())
        .unwrap_or(text.len() / 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_read() {
        let mut store = ContextStore::new();
        store.write("key".to_string(), "value".to_string());
        assert_eq!(store.read("key"), Some("value"));
    }

    #[test]
    fn read_missing_returns_none() {
        let store = ContextStore::new();
        assert_eq!(store.read("missing"), None);
    }

    #[test]
    fn delete_existing() {
        let mut store = ContextStore::new();
        store.write("key".to_string(), "value".to_string());
        assert!(store.delete("key"));
        assert_eq!(store.read("key"), None);
    }

    #[test]
    fn delete_missing_returns_false() {
        let mut store = ContextStore::new();
        assert!(!store.delete("missing"));
    }

    #[test]
    fn list_sorted_with_token_counts() {
        let mut store = ContextStore::new();
        store.write("bravo".to_string(), "some content".to_string());
        store.write("alpha".to_string(), "other content".to_string());

        let entries = store.list();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "alpha");
        assert_eq!(entries[1].0, "bravo");
        // Token counts should be positive
        assert!(entries[0].1 > 0);
        assert!(entries[1].1 > 0);
    }

    #[test]
    fn overwrite_replaces_value() {
        let mut store = ContextStore::new();
        store.write("key".to_string(), "old".to_string());
        store.write("key".to_string(), "new".to_string());
        assert_eq!(store.read("key"), Some("new"));
    }

    #[test]
    fn token_count_is_positive_for_nonempty_text() {
        let count = estimate_tokens("hello world");
        assert!(count > 0);
    }
}
