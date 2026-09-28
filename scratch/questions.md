# Open questions and assumptions (LOO-330)

## Assumptions made to keep moving

- **No message was sent to the LOO-329 or LOO-331 workers.** Both were running
  kickoff with empty `scratch/` on 2026-09-28. Posting a Linear comment would
  steer a running worker under an unclear author, so the proposed shared
  vocabulary and boundary table live in the design for reconciliation at
  review-design.
- **Token figures are characters ÷ 4.** The tokenizer was unavailable in this
  environment. LOO-331 owns the measured numbers.
- **"Held" and "operated" are proposed words.** Jack has not chosen them.
- **`lf memory` reads do not reverse the "no CLI surface for memory" decision**
  because they never write or cache. Jack may read that decision more strictly.

## For Jack at review-design

1. May agents create held subwaves during curation, or only people?
2. Should held subwaves appear in the sidebar?
3. Keep leaf shorthand (`release`), or require qualified addresses always?
4. "Subwave" or "area" for a scope that operates nothing?
5. Short stable headings as entry addresses, or keep dated headings?
6. Is a child index enough for a parent Run, or do some skills need full union
   by default?

## For LOO-329

- Does `parent_wave_id` derive from the path or merely have to agree with it?
- Where does a held scope's identity live when it has no registry row?
- Where is Task placement stored?

## For LOO-331

- Which skills read child memory today by following links, and how often?
- What does a release-focused prompt contain now, measured?
