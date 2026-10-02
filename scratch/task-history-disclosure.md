# Task history disclosure research · 2026-10-02

Jack Heart found the header integration better but called the All button awkward
and requested examples of progressively disclosed controls. This is historical research. Jack Heart subsequently approved the inline
checkbox/number prototype; the menu below is superseded.

Observed in official product documentation:
- Linear Display options puts completed-project visibility and recency in view
  settings, with week/month/year, all, and none choices:
  https://linear.app/docs/display-options#completed-projects
- Notion search reveals date presets (Today, Last 7 days, Last 30 days) and
  manual dates after selecting its Date filter:
  https://www.notion.com/help/search
- Grafana displays the current time range as the trigger. Clicking it reveals
  relative presets and custom ranges:
  https://grafana.com/docs/grafana/latest/visualizations/dashboards/use-dashboards/

Interpretation: All time is a choice of range, not an independent boolean worth
its own persistent button. Numeric editing is another level of detail.

Proposed header interaction:
1. Initially show only the existing Show completed toggle.
2. Enabling it reveals one adjacent range menu, labeled Last 7 days.
3. That menu offers Last 7 days, Last 30 days, All time, and Custom….
4. Custom reveals the existing positive-day input in a popover; invalid input
   keeps the applied range unchanged. Dismiss/Cancel preserves the prior range.
5. The closed range menu summarizes the selected value (including Last N days).
   Turning Show completed off hides its range control and retains the selection.

Preserve successful-completion semantics, seven-day initial default, arbitrary
positive N, All time, counts, and exclusion of canceled duplicates. A calendar
would add a new absolute-date concept outside this Task's scope.

The sources document behavior; no claim is made that their current applications
were exercised interactively. No implementation changes were made for this study.
