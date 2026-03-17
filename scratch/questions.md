# Open questions

- Kept a deprecated `auth.provider` YAML alias for `auth.mode` during the rename so old configs do not silently fall back to `local`. If we want a hard break instead, remove the serde alias and add explicit validation.
