# Open questions

- If the same provider/login exists both locally and through the broker, is it
  one logical account with two credential sources, or an ambiguity that needs a
  source-qualified selector?
- Does outer `--only-account` restrict the merged local-plus-forwarded catalog,
  or only the accounts the origin broker exposes?
- How do outer preferences compose with an inner `--account` preference over
  the merged catalog?
- When both the target and origin have declared routes, which route supplies
  the first fallback after explicit account preferences?
- What public spelling should own placement-aware orchestration: `lf home
  start`, `lf home-start`, or a later command? The base `lf start` remains local.
