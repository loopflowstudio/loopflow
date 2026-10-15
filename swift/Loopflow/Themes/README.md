# Bundled themes

Choose **Loopflow Light** for original cream/burgundy, **Loopflow Dark** for the
original slate design with bronze instead of pink, **Linen** for a quiet light
green/neutral palette, or **Hearth** for warm charcoal. The other 20 slots span
rose, violet, amber, green, teal and blue accents. Ordinary headings stay quiet.

All 24 slots support Edit, rename, import replacement and Reset. Reset restores
the original bundled record, including its name. Apply/Cancel covers all slots
and both assignments together. Complete replacements live in appearance
preferences; source JSON remains the Reset baseline. `Migrations/` holds only
retired selected presets for one-time conversion, never extra catalogue slots.

Every JSON file supplies a complete Loopflow and terminal palette. Missing roles
fail decoding; unreadable roles fail validation. `Sources.md` records original
terminal colors and marked cursor/selection adaptations. `LICENSE` and `Licenses/`
retain upstream notices. App palettes are independently curated complete records.
No source Ghostty file or authoring conversion runs during catalogue loading.

`GhosttyThemeMapping` converts imports once: ANSI 4 seeds action/focus and Working,
3 Waiting, 1 failure and 2 success. It quiets app text, builds lighter dark
component surfaces and derives Work selection separately from terminal selection.
It never changes imported terminal colors or infers state. Near middle gray,
contrast may limit the surface or text hierarchy.

`LoopflowPalette` aliases focus to accent ink, ordinary links/actions to text,
and status stroke/text to status ink. Solid buttons own distinct base/hover/pressed
fills. Ghostty derives terminal cell fills from the same record's terminal colors;
Swift owns gutter/header decoration. Every current field is required. Both
SwiftPM and Xcode ship the catalogue offline.
