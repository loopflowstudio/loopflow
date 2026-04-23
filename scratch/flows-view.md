# Flows View (shipped)

Static catalog for the Concerto Flows tab. Two panes: catalog (left, grouped by build/govern/ops) and "used by" (right, upward walk from the selected flow or step). Backed by a new `GET /catalog` endpoint on `lfd`. Full design captured in `wave/flows/README.md` and the session-state follow-on item.

## Try it

1. Launch Concerto (macOS) against a local `lfd`.
2. Open the **Flows** tab.
3. Expand `build` → `build`. Confirm it expands into `kickoff → review-design → loop(...) → deploy` with the xor and loop containers rendered.
4. Click `gate`. Right pane shows every flow and parent flow that reaches it. Click any breadcrumb to re-select.
5. Drop a flow YAML in `.lf/flows/` and reload — the repo version appears with an override accent.

## Verify

```bash
# Catalog endpoint responds with resolved builtin + repo content
curl -s localhost:PORT/catalog | jq '.result.flows | map(.name)'
curl -s localhost:PORT/catalog | jq '.result.steps | length'

# Swift DTO round-trips
swift test --package-path swift --filter CatalogTests

# Concerto UI test
cd swift && xcodegen generate && \
  xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto \
    -destination 'platform=macOS' \
    CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Expected: endpoint returns both builtin and any `.lf/flows/*` overrides, Swift tests pass, UI tests pass.

## Follow-on work

Tracked in `wave/flows/`:
- Session-state overlay (item 2) — the you-are-here dot on top of this map.
- `maybe` primitive (item 3) — simplifies xor rendering once `xor(_, silence)` migrates.
- iOS layout, search/filter, xor label polish (item 4).
