# Interactive Checkpoints — Validation

## Try it

```bash
# Start a build flow that reaches review-design
lfq run engbot

# Check that the queue surfaces the waiting review
curl -s localhost:7475/attention | jq '.[] | select(.kind == "interactive")'

# Complete the session and verify the item resolves
curl -s localhost:7475/attention | jq '.[] | select(.status == "resolved")'
```

## Expected

- `review-design` and `wave/review` appear as `interactive` attention items
- Design review shows an inline design preview instead of raw JSON
- Wave review shows the mutation summary inline
- Completing the terminal session resolves the attention item

## Measure

**Before:** Interactive checkpoints produced zero attention items. The conductor had to notice waiting waves from sidebar status alone.

**After:** Every `WaitInteractive` step produces exactly one interactive attention item, with typed detail and explicit resolution on terminal completion.

Target: 100% of unresolved human checkpoints represented as attention items.
