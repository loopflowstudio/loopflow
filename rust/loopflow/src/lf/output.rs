#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub cyan: &'static str,
    pub bold: &'static str,
    pub dim: &'static str,
    pub yellow: &'static str,
    pub green: &'static str,
    pub reset: &'static str,
}

impl Colors {
    pub fn new() -> Self {
        let use_color = std::env::var("NO_COLOR").is_err() && atty::is(atty::Stream::Stdout);
        if use_color {
            Self {
                cyan: "\x1b[36m",
                bold: "\x1b[1m",
                dim: "\x1b[90m",
                yellow: "\x1b[33m",
                green: "\x1b[32m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                cyan: "",
                bold: "",
                dim: "",
                yellow: "",
                green: "",
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

use crate::engine::prompt::{ContextBreakdown, DiffTier};

/// Format the context header table for stderr output.
pub fn format_context_header(breakdown: &ContextBreakdown, budget: usize) -> String {
    let mut lines = Vec::new();

    // Step name as prominent header
    let title = breakdown.step_name.as_deref().unwrap_or("context");
    let bar_len = 45usize.saturating_sub(title.len() + 4);
    lines.push(format!(
        "\u{2500}\u{2500} {} {}",
        title,
        "\u{2500}".repeat(bar_len)
    ));

    // Step tokens
    lines.push(format_row("step", breakdown.step, ""));

    // Direction
    let dir_detail = if breakdown.direction_names.is_empty() {
        "\u{2014}".to_string()
    } else {
        breakdown.direction_names.join(", ")
    };
    lines.push(format_row("direction", breakdown.direction, &dir_detail));

    // System
    lines.push(format_row("system", breakdown.system, "loopflow"));

    // Diff
    let diff_detail = match breakdown.diff_tier {
        DiffTier::UnifiedDiff => format!("unified ({} files)", breakdown.diff_file_count),
        DiffTier::StatOnly => format!("stat ({} files)", breakdown.diff_file_count),
        DiffTier::None => "\u{2014}".to_string(),
    };
    lines.push(format_row("diff", breakdown.diff, &diff_detail));

    // Docs
    if breakdown.doc_count > 0 {
        let docs_detail = format!("{} files", breakdown.doc_count);
        lines.push(format_row("docs", breakdown.docs, &docs_detail));
    }

    // Area
    if breakdown.area_doc_count > 0 {
        let area_detail = match &breakdown.area_name {
            Some(name) => format!("{} ({} files)", name, breakdown.area_doc_count),
            None => format!("{} files", breakdown.area_doc_count),
        };
        lines.push(format_row("area", breakdown.area, &area_detail));
    }

    // Wave
    if let Some(ref wave) = breakdown.wave_name {
        lines.push(format_row("wave", 0, wave));
    }

    // Clipboard
    if breakdown.has_clipboard {
        lines.push(format_row("clipboard", breakdown.clipboard, ""));
    }

    // Separator + total
    lines.push(format!("  {}", "\u{2500}".repeat(35)));
    let total = breakdown.total();
    let pct = if budget > 0 {
        (total * 100) / budget
    } else {
        0
    };
    lines.push(format!(
        "  {:<12} {:>6}  {}% of {}k",
        "total",
        format_tokens(total),
        pct,
        budget / 1000,
    ));

    lines.join("\n")
}

fn format_row(label: &str, tokens: usize, detail: &str) -> String {
    format!("  {:<12} {:>6}  {}", label, format_tokens(tokens), detail)
}

fn format_tokens(n: usize) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Build a reproducible lf command from run parameters.
pub fn format_reproducible_command(
    step: Option<&str>,
    directions: &[String],
    wave: Option<&str>,
    area: Option<&str>,
    clipboard: bool,
    model: Option<&str>,
) -> String {
    let mut parts = vec!["lf".to_string()];
    if let Some(s) = step {
        parts.push(s.to_string());
    }
    for d in directions {
        parts.push(format!("-d {}", d));
    }
    if let Some(w) = wave {
        parts.push(format!("--wave {}", w));
    }
    if let Some(a) = area {
        parts.push(format!("-a {}", a));
    }
    if clipboard {
        parts.push("-c".to_string());
    }
    if let Some(m) = model {
        // Only include if not the default
        parts.push(format!("-m {}", m));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_basic() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(42), "42");
        assert_eq!(format_tokens(1234), "1,234");
        assert_eq!(format_tokens(75000), "75,000");
        assert_eq!(format_tokens(123456), "123,456");
    }

    #[test]
    fn format_context_header_empty() {
        let breakdown = ContextBreakdown::default();
        let header = format_context_header(&breakdown, 75_000);
        assert!(header.contains("\u{2500}\u{2500} context \u{2500}"));
        assert!(header.contains("total"));
        assert!(header.contains("0% of 75k"));
    }

    #[test]
    fn format_context_header_with_content() {
        let breakdown = ContextBreakdown {
            step: 1000,
            direction: 500,
            system: 3000,
            diff: 5000,
            docs: 2000,
            step_name: Some("implement".to_string()),
            direction_names: vec!["product-engineer".to_string()],
            diff_tier: DiffTier::UnifiedDiff,
            diff_file_count: 8,
            doc_count: 3,
            ..Default::default()
        };
        let header = format_context_header(&breakdown, 75_000);
        assert!(header.contains("\u{2500}\u{2500} implement \u{2500}"));
        assert!(header.contains("product-engineer"));
        assert!(header.contains("unified (8 files)"));
        assert!(header.contains("3 files"));
        assert!(header.contains("15% of 75k"));
    }

    #[test]
    fn format_reproducible_command_minimal() {
        let cmd = format_reproducible_command(Some("debug"), &[], None, None, false, None);
        assert_eq!(cmd, "lf debug");
    }

    #[test]
    fn format_reproducible_command_full() {
        let cmd = format_reproducible_command(
            Some("implement"),
            &["product-engineer".to_string()],
            Some("rust"),
            Some("src/"),
            true,
            Some("claude:opus"),
        );
        assert_eq!(
            cmd,
            "lf implement -d product-engineer --wave rust -a src/ -c -m claude:opus"
        );
    }
}
