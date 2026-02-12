# Visual Design Research

Reference patterns from apps that do sidebar + status + detail well. Organized by focus area for use in the design audit (04-design-audit) and polish work (Phase 4).

**Reference apps**: Linear, Notion, Arc, Figma

**Why these four**: Linear — closest analogue (status-grouped sidebar, high density, reduced noise). Notion — sidebar hierarchy and progressive disclosure. Arc — context switching and color-coded grouping. Figma — collaboration indicators and panel layout.

---

## 1. Sidebar Patterns

### Hierarchy and grouping

**Linear** groups items by status category (Backlog > Unstarted > Started > Completed > Canceled). Section headers are minimal — small, uppercase, low-opacity text. Sub-menus appear on hover rather than cluttering the sidebar permanently. The redesign specifically reduced visible items per section, condensing navigation into fewer top-level entries.

**Notion** uses accordion sections with chevron disclosure. 224px fixed width. Two-level maximum hierarchy recommended — deeper nesting causes indentation overflow and truncated labels. Teamspaces provide top-level grouping, with pages nested within.

**Arc** uses three-tier hierarchy: always-visible favorites (icon-only), space-pinned tabs, and temporary tabs (auto-expire after 12 hours). This tiering by permanence maps directly to Concerto's wave lifecycle.

**Figma UI3** moved to a linear navigation panel: file > branch > project > pages > layers. No nested accordion — flat sequential listing with logical extension points.

**Pattern for Concerto**: The current grouping (Active > Idle with Needs Attention/Open PRs subsections) is sound. Linear validates status-based grouping. But Concerto's section headers ("Active", "Idle") could be less prominent — Linear uses smaller, lower-opacity section labels to avoid visual competition with the items themselves. The sidebar should emphasize waves, not categories.

### Density

**Linear** achieves high density by: (1) condensing secondary info onto the same line as the primary label, (2) showing sub-menus on hover instead of always-visible, (3) tight vertical spacing between items with generous horizontal padding.

**Notion** achieves density through collapsible sections — users control what's visible. Default state is mostly collapsed.

**Pattern for Concerto**: Concerto's wave rows show name + flow badge on line 1, area + iteration + status on line 2. This two-line pattern is good — matches Linear's approach. But the flow badge (capsule pill) takes significant horizontal space. Consider whether it earns that weight or could be reduced to text.

### Selection and hover

**Linear** uses subtle background highlight on hover and slightly stronger highlight on selection. No border, no bold outline — just background opacity change.

**Arc** uses a rounded rect selection indicator with the space's accent color.

**Notion** uses a light background tint on hover, slightly darker on selection.

**Pattern for Concerto**: Current selection (white @ 0.2 background on burgundy sidebar) works. The hover state (white @ 0.08) is appropriately subtle. This matches the reference apps. No change needed.

---

## 2. Status Visualization

### Status icons and colors

**Linear** uses geometric icons for status: empty circle (backlog), half-circle (unstarted), partially-filled circle (in progress), filled circle (done), crossed circle (canceled). Each status category has an associated color (customizable per team). The icons are small and inline — they don't dominate the row.

**Figma** uses a "Changed" badge on canvas sections that require developer attention. Status is contextual — it appears when relevant and disappears when resolved.

**Pattern for Concerto**: Concerto uses colored circles (green = running, yellow/orange = waiting, gray = idle). This is simpler than Linear's geometric vocabulary but effective. The key insight from Linear: status icons should be *small and inline*, not the visual anchor of the row. Concerto's current 6pt circles achieve this. The wave name should be the visual anchor — which it currently is (medium weight, full opacity white).

### Status in context

**Linear** orders items closest-to-done first in list views. Completed and canceled items trail. This is a deliberate prioritization choice — surface what's actionable.

**Arc** auto-expires unused tabs, surfacing only what's active. The permanence tier (favorites > pinned > temporary) serves as implicit status.

**Pattern for Concerto**: Concerto's grouping already does this — "Needs Attention" and "Active" appear before "Idle". The ordering within groups could be refined: within Active, waves closest to shipping (most iterations, has PR) could surface first. This is a data ordering issue, not a visual one.

### Urgency and attention

**Linear** doesn't use animation or pulsing for urgency. Color alone (warning orange, error red) signals priority. The lack of motion is intentional — in a dense sidebar, animation is distracting noise.

**Arc** uses no urgency indicators beyond favicon badges (inherited from websites).

**Pattern for Concerto**: The "PR limit" label in warning yellow (visible in the crystal-melody row) is an effective urgency signal without being noisy. This is already well-executed. Resist adding animation or pulsing to status indicators.

---

## 3. Color Usage

### Sidebar background treatment

**Linear** uses a dark sidebar (near-black in dark mode, light gray in light mode) distinct from the content area. The sidebar is visually recessive — it's infrastructure, not content.

**Notion** uses a slightly tinted sidebar background (warm gray) distinct from the white content area.

**Arc** gives each space its own background color, making the sidebar the most visually distinctive element. This is appropriate because Arc's sidebar IS the product — in Concerto, the detail pane is the product.

**Pattern for Concerto**: The burgundy sidebar is a strong brand choice. It differs from all reference apps (which use neutral backgrounds) but aligns with VISUAL_DESIGN.md's brand philosophy ("warmth, craft, classical instruments"). The risk: burgundy competes for attention with status colors. The mitigation: use white/cream text at varying opacities (already done) and keep status colors small (6pt circles). The burgundy sidebar works as long as the detail pane reads as the primary workspace.

### Accent and status color separation

**Linear** migrated to LCH color space for perceptual uniformity. Status colors are separate from the accent color — the accent identifies the app/team, while status colors convey meaning. Three core variables (base, accent, contrast) generate the entire theme.

**Figma** uses minimal accent color in the chrome — blue for selection, otherwise neutral. Status badges use distinct colors (green, orange, red) that don't overlap with the accent.

**Pattern for Concerto**: VISUAL_DESIGN.md defines burgundy as accent and separate status tokens (success green, warning gold, error orange, info cyan). This separation is correct. Current deviation: the neon green status indicator for "running" is too bright against the burgundy sidebar — it should use the VISUAL_DESIGN.md `success` token (#2D6A4F, a muted forest green) instead of a saturated neon green. This is the single highest-leverage color fix.

### Light/dark mode

**Linear** generates themes from three variables, ensuring both modes feel native. The sidebar darkens further in dark mode — it's always darker than the content area.

**Pattern for Concerto**: The cream (light) and slate (dark) palettes in VISUAL_DESIGN.md are well-defined. The burgundy sidebar should remain constant across modes (it's dark enough for both). Verify that status colors remain legible against burgundy in both modes.

---

## 4. Typography

### Heading and body separation

**Linear** uses Inter Display for headings and Inter Regular for body — same family, different optical sizes. This creates subtle hierarchy without jarring font switches.

**Figma** uses their custom figmaSans with variable weight and width, plus figmaMono for code. The variable font allows smooth weight transitions for hierarchy.

**Pattern for Concerto**: VISUAL_DESIGN.md specifies Cormorant Garamond (serif) for headings and Lato (sans) for body. This is a bolder choice than Linear/Figma — the serif/sans contrast creates stronger hierarchy but risks feeling "designed" rather than "functional." In the sidebar, this is working well: wave names in Lato (medium weight) read clearly. The Cormorant italic for activity timestamps is a nice touch that follows VISUAL_DESIGN.md's "reserved for special moments" guidance.

### Monospace mixing

**Linear** uses monospace sparingly — issue IDs, code references. It's never used for primary content.

**Figma** uses monospace for labels, badge text, and code — more prominent than Linear but still secondary to the sans-serif body.

**Pattern for Concerto**: Concerto uses JetBrains Mono for commit SHAs, diff stats, and live output. This is correct — monospace for machine-generated content, sans-serif for human content. The live output area (10pt monospace, black @ 0.3 background) follows this pattern well. No change needed.

### Information density through typography

**Linear** achieves density by using a single font size for most sidebar content and varying weight/opacity for hierarchy. Bold = primary, regular = secondary, light opacity = tertiary.

**Pattern for Concerto**: Current approach (medium weight name, caption secondary info, caption2 badges) uses three size tiers. Linear suggests two would suffice: same size, varied weight/opacity. Consider whether the badge text (caption2) could be the same size as the secondary info (caption) with lower opacity instead of smaller size.

---

## 5. Spacing

### Section separation

**Linear** uses minimal section dividers — a small gap plus a subtle header label. No visible divider lines between sections.

**Notion** uses slightly more visible separation — thin dividers and section headers.

**Figma** uses no dividers in UI3's layer panel. Sections are separated by spacing alone.

**Pattern for Concerto**: The thin divider between "Active" and "Idle" sections (white @ 0.1) is appropriately subtle. Matches Linear's approach. The section headers ("Active", "Idle") could be even more subdued — currently caption2 + medium weight. Linear uses smaller, lower-opacity labels.

### Row density

**Linear** sidebar rows are approximately 28-32px tall with 4-8px vertical spacing between items. Content is tightly packed but each row has enough internal padding to remain clickable.

**Notion** sidebar rows are slightly taller (32-36px) with 2-4px spacing.

**Pattern for Concerto**: Current rows use 8pt vertical padding + 4pt spacing between items. Total row height is approximately 48-56px (two text lines + padding). This is lower density than Linear's single-line rows but appropriate for Concerto's two-line format (name + metadata). The density is good for a tool monitoring autonomous agents — you need to see status at a glance without hunting.

### Card and panel padding

**Linear** uses 16px padding in panels and cards. Consistent across list views, detail views, and settings.

**Figma** uses 24px gutters in their blog layout, 16px in the app panels.

**Pattern for Concerto**: Current detail panel uses 20px horizontal padding (Spacing.xl). This is slightly more generous than Linear but appropriate for the wider detail pane. The progress card uses 16px padding (Spacing.lg) with 12px corner radius — matches Linear's patterns.

---

## Concerto-Specific Observations

Patterns from screenshots that don't map to any reference app — unique to Concerto's domain:

### The detail pane is too empty

When a wave is running, the detail pane shows "Progress" with a spinner and "Live Output" with placeholder text. This is 90% empty space. Linear fills its detail views with contextual information — related issues, activity history, metadata. Concerto should show more about the wave: flow step progress, recent commits, area scope, direction, last activity. The `StepRunner` and `FlowProgressPills` components exist but aren't visible in the running state screenshots.

### Action buttons are prominent but sparse

"Clone" and "Stop" buttons use filled burgundy backgrounds (DarkButtonStyle), making them visually heavy. Linear uses ghost buttons (text only, no fill) for secondary actions and reserves filled buttons for primary CTAs. Consider: "Stop" is destructive — it should feel accessible but not inviting. A ghost/outline button style might be more appropriate.

### The sidebar communicates well

The burgundy sidebar with white text, status circles, flow badges, and grouped sections is Concerto's strongest visual element. The information density is good — you can scan 4 waves and understand their state in under 2 seconds. This matches the conductor persona's need to "see what needs attention without drilling in."

---

## Highest-Leverage Findings

Ordered by impact for the design audit:

1. **Replace neon green status with VISUAL_DESIGN.md success token** (#2D6A4F). The bright green clashes with the burgundy sidebar and breaks the warm color palette.

2. **Fill the detail pane.** Show flow progress pills, commits, diff stats, and wave config when a wave is running — not just a spinner. The components exist; surface them.

3. **Reduce action button visual weight.** Ghost buttons for secondary actions (Clone), filled for primary (Run). Outline/destructive style for Stop.

4. **Subdue section headers.** Smaller, lower-opacity section labels in the sidebar. The waves are the content, not the categories.

5. **Consider badge density.** Flow badges (capsule pills) take significant horizontal space. Could be text-only at lower opacity.
