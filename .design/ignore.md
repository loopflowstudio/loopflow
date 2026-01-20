# ignore: alias for exclude

## What to build

Add `ignore` as an alias for `exclude` in `.lf/config.yaml`. Both keys work, values are merged.

## Data structures

```python
# config.py
class Config(BaseModel):
    exclude: list[str] = Field(default_factory=list)
    ignore: list[str] = Field(default_factory=list)  # alias, merged with exclude

    @model_validator(mode="after")
    def merge_ignore_into_exclude(self) -> "Config":
        """Merge ignore into exclude (ignore is an alias)."""
        if self.ignore:
            self.exclude = list(set(self.exclude + self.ignore))
            self.ignore = []  # clear after merge
        return self
```

## Key functions

No new functions. Just the validator that merges on load.

## Constraints

- `ignore` and `exclude` must merge, not override
- Order doesn't matter (both are glob patterns)
- Empty `ignore` is valid (no-op)
- Don't warn about using `ignore`—it's a supported alias, not deprecated

## Done when

```bash
# In a repo with this config:
# .lf/config.yaml
# ignore:
#   - uv.lock

cd /Users/jack/src/loopflowstudio.mobile
lf review -c 2>&1 | grep -i "uv.lock"
# Should return nothing (uv.lock excluded from context)
```
