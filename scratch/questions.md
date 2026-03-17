# Open questions

- Asana does not expose a numeric rank field, so `AsanaClient::update_item` currently ignores `PmItemUpdate.rank` and treats rank-only updates as a no-op. If later waves need remote reordering, they should add `insert_before` / `insert_after` support instead of trying to force a numeric rank into the API.
