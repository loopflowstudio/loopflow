# Three visual polish directions for the built native workspace

Jack asked: "Prototype 3 different visual design polishes on the work in this branch", and wants to see them.
Bounded contribution. Write ONLY inside scratch/visual-study/polish/. No commits, native Swift edits, PM, providers, installs.

Subject is the native build as it exists NOW (cycles 1–4), not the older study. Look at these native captures first:
- scratch/implementation-cycle/cycle-03-evidence/{wave-false,Full hierarchy-false,Compact-false,Sessions-false}.png
- scratch/implementation-cycle/cycle-04-review-evidence/real-feature-*.png (actual Feature Flow diagram)
- scratch/implementation-cycle/cycle-04-evidence/task-flow-running.png
Governing design: scratch/main-view-task.md (Accepted experience + Flow sections). Accepted reference remains scratch/visual-study/mockups.html (A frame) and flow-options.* (Flow C). Keep structure, hierarchy, content and interactions; this is POLISH (spacing, type scale, color, rhythm, state treatment, density), not a new layout or information model.

Observed rough edges to address (each direction solves them its own way):
- Wave page: raw warning line (e.g. "W2-127: Task's owning Project is absent … lf work abandon task …") dominates; metrics block visually heavier than Tasks; section heading styles inconsistent (small caps CURRENT KRS / TASKS vs serif "Metrics"); Chapter history link floats.
- Sidebar: selected Wave pill is heavy; Task row truncation and Session count glyph are cramped; header menu glyph weak.
- Flow diagram: 13 nodes overflow at ~1100pt so the queue→land tail is off-screen; two return arrowheads collide at implement; nested loop regions stack two "Loop · Iteration 3" labels (per-loop tuple is the accepted fix: each loop labels its own count, header shows e.g. "Iteration (2, 1)"); completed green reads grey under the blue loop tint.
- Task page: status line, Flow, Sessions, Description, Comments(n) need a clear vertical rhythm.

Build one self-contained page scratch/visual-study/polish/index.html (plus optional polish.css/js in that dir) with a direction switcher A/B/C (query ?v=a|b|c, default a) and a surface switcher: Wave page, Task (running, Feature flow, 2 open Sessions), Session drill-down (breadcrumb Wave / Task / Session name, dark terminal pane + companion). Same sample data in all three; label it "Sample data" once, discreetly.
Three materially different polish directions, all within brand (burgundy #722F37, cream #FAF8F5, charcoal terminal #2B3036; fonts via ../../../swift/Loopflow/Fonts/: CormorantGaramond, Lato, JetBrainsMono):
A. Quiet editorial — refine what exists: tighter serif/sans scale, one heading system, hairline rules, warnings as a compact dismissible inline note with Details disclosure, calmer sidebar selection.
B. Crisp tool — denser, more app-like: smaller type, aligned grid columns (IDs, states), compact Flow that fits 13 nodes via smaller chips + wrapped tail row, stronger state color chips.
C. Warm spacious — generous whitespace, larger Flow with two clearly separated loop lanes and distinct arrowheads, soft cards only where they carry meaning (metrics), burgundy used as accent not fill.
Flow colors stay: blue loops/running, green completed, yellow pending human, red blocked, neutral stopped. Literal lowercase mono skill names. No Pause. Resume/Stop & restart available.
Controls work locally (select Task/Session, expand Description/Comments/warning details, hover Stop & restart). Accessible buttons, visible focus. Fit 1440x900 and 1100x800 without page-level horizontal scroll.
Finish by writing scratch/visual-study/polish/README.md: what each direction changes and why, and remaining limits. Do not open a browser.
