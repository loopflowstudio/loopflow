/// A ledger identity shortened for the console (ids correlate by prefix).
/// Shared by the run, exec, and trace surfaces.
pub(crate) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}
