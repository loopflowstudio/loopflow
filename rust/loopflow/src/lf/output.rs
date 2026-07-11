use std::io::IsTerminal;

#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub cyan: &'static str,
    pub bold: &'static str,
    pub dim: &'static str,
    pub yellow: &'static str,
    pub green: &'static str,
    pub red: &'static str,
    pub reset: &'static str,
}

impl Colors {
    pub fn new() -> Self {
        let use_color = std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal();
        if use_color {
            Self {
                cyan: "\x1b[36m",
                bold: "\x1b[1m",
                dim: "\x1b[90m",
                yellow: "\x1b[33m",
                green: "\x1b[32m",
                red: "\x1b[31m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                cyan: "",
                bold: "",
                dim: "",
                yellow: "",
                green: "",
                red: "",
                reset: "",
            }
        }
    }
}

impl Default for Colors {
    fn default() -> Self {
        Self::new()
    }
}

use crate::engine::prompt::PromptComponents;
use crate::trace::PreparedTurnContext;

/// Format the context header table for stderr output.
pub fn format_context_header(
    context: &PreparedTurnContext,
    components: &PromptComponents,
) -> String {
    use std::collections::BTreeMap;

    let mut lines = Vec::new();
    let title = components
        .skill
        .as_ref()
        .map(|skill| skill.name.as_str())
        .unwrap_or("context");
    let bar_len = 45usize.saturating_sub(title.len() + 4);
    lines.push(format!(
        "\u{2500}\u{2500} {} {}",
        title,
        "\u{2500}".repeat(bar_len)
    ));

    let mut kinds: BTreeMap<&str, (u64, usize)> = BTreeMap::new();
    for asset in context.assets() {
        let entry = kinds.entry(asset.kind.as_str()).or_default();
        entry.0 += asset.attributed_tokens;
        entry.1 += 1;
    }
    for (kind, (tokens, count)) in kinds {
        if tokens > 0 {
            lines.push(format_row(
                kind,
                tokens as usize,
                &format!("{count} assets"),
            ));
        }
    }

    if let Some(ref wave) = components.wave {
        lines.push(format_row("scope", 0, wave));
    }

    // Separator + total
    lines.push(format!("  {}", "\u{2500}".repeat(35)));
    lines.push(format_row("total", context.total_tokens() as usize, ""));

    lines.join("\n")
}

fn format_row(label: &str, tokens: usize, detail: &str) -> String {
    format!(
        "  {:<12} {:>6}  {}",
        label,
        format_int(tokens as u64),
        detail
    )
}

// -- Table cells --------------------------------------------------------------
// Keep reader output aligned from the visible cell contents. This is the same
// shape used by tree readers such as `lf wt list`: one record per line, enough
// spacing to scan it, and no syntax that an agent has to strip first.

/// Measure a left-aligned column from its header and visible cell contents.
pub fn column_width<I, S>(header: &str, values: I) -> usize
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    values
        .into_iter()
        .map(|value| value.as_ref().chars().count())
        .fold(header.chars().count(), usize::max)
}

/// `1234567` -> `1,234,567`.
pub fn format_int(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

/// Clip to `width` characters, marking the clip with an ellipsis.
pub fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let head: String = value.chars().take(width.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

pub fn format_cost(value: f64) -> String {
    format!("${value:.2}")
}

/// Build a reproducible lf command from run parameters.
pub fn format_reproducible_command(
    skill: Option<&str>,
    directions: &[String],
    wave: Option<&str>,
    docs: &[String],
    clipboard: bool,
    no_loopflow: bool,
    model: Option<&str>,
) -> String {
    let mut parts = vec!["lf".to_string()];
    if let Some(s) = skill {
        parts.push(s.to_string());
    }
    for d in directions {
        parts.push(format!("-d {}", d));
    }
    if let Some(w) = wave {
        parts.push(format!("--wave {}", w));
    }
    for doc in docs {
        parts.push(format!("--docs {}", doc));
    }
    if clipboard {
        parts.push("-c".to_string());
    }
    if no_loopflow {
        parts.push("--no-loopflow".to_string());
    }
    if let Some(m) = model {
        parts.push(format!("-m {}", m));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_int_groups_thousands() {
        assert_eq!(format_int(0), "0");
        assert_eq!(format_int(42), "42");
        assert_eq!(format_int(1234), "1,234");
        assert_eq!(format_int(1_234_567), "1,234,567");
    }

    #[test]
    fn column_width_fits_header_and_owned_cells() {
        assert_eq!(column_width("STATUS", ["open", "done"]), 6);
        assert_eq!(
            column_width(
                "TITLE",
                vec!["short".to_string(), "a longer title".to_string()]
            ),
            14
        );
    }

    #[test]
    fn truncate_adds_ellipsis_past_width() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd\u{2026}");
    }

    #[test]
    fn format_cost_always_shows_cents() {
        assert_eq!(format_cost(0.0), "$0.00");
        assert_eq!(format_cost(61.851), "$61.85");
    }

    #[test]
    fn format_context_header_empty() {
        let context = crate::trace::PreparedTurnContext::from_prompts("", "");
        let header = format_context_header(&context, &PromptComponents::default());
        assert!(header.contains("\u{2500}\u{2500} context \u{2500}"));
        assert!(header.contains("total"));
        assert!(header.contains("0"));
    }

    #[test]
    fn format_context_header_with_content() {
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "task");
        let components = PromptComponents {
            skill: Some(crate::engine::Skill::named("implement")),
            wave: Some("intelligence".to_string()),
            ..Default::default()
        };
        let header = format_context_header(&context, &components);
        assert!(header.contains("\u{2500}\u{2500} implement \u{2500}"));
        assert!(header.contains("assembly"));
        assert!(header.contains("intelligence"));
    }

    #[test]
    fn format_context_header_total_comes_from_exact_manifest() {
        let context = crate::trace::PreparedTurnContext::from_prompts("one", "two");
        let header = format_context_header(&context, &PromptComponents::default());
        assert!(header.contains(&format_int(context.total_tokens())));
    }

    #[test]
    fn format_reproducible_command_minimal() {
        let cmd = format_reproducible_command(Some("debug"), &[], None, &[], false, false, None);
        assert_eq!(cmd, "lf debug");
    }

    #[test]
    fn format_reproducible_command_full() {
        let cmd = format_reproducible_command(
            Some("implement"),
            &["security".to_string()],
            Some("rust"),
            &["src/".to_string()],
            true,
            true,
            Some("claude:opus"),
        );
        assert_eq!(
            cmd,
            "lf implement -d security --wave rust --docs src/ -c --no-loopflow -m claude:opus"
        );
    }
}
