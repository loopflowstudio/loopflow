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

use crate::engine::prompt::{ContextBreakdown, DiffTier, DocumentSource};

/// Format the context header table for stderr output.
pub fn format_context_header(breakdown: &ContextBreakdown, budget: usize) -> String {
    let mut lines = Vec::new();
    let step_tokens = breakdown.source_tokens(DocumentSource::Step);
    let direction_tokens = breakdown.source_tokens(DocumentSource::Direction);
    let scratch_tokens = breakdown.source_tokens(DocumentSource::Scratch);
    let wave_tokens = breakdown.source_tokens(DocumentSource::Wave)
        + breakdown.source_tokens(DocumentSource::Summary);
    let wave_file_count = breakdown.source_count(DocumentSource::Wave)
        + breakdown.source_count(DocumentSource::Summary);
    let diff_tokens = breakdown.source_tokens(DocumentSource::Diff);
    let docs_tokens = breakdown.source_tokens(DocumentSource::RepoDoc);
    let wave_memory_tokens = breakdown.source_tokens(DocumentSource::WaveMemory);
    let area_tokens = breakdown.source_tokens(DocumentSource::Area);
    let clipboard_tokens = breakdown.source_tokens(DocumentSource::Clipboard);

    // Step name as prominent header
    let title = breakdown.step_name.as_deref().unwrap_or("context");
    let bar_len = 45usize.saturating_sub(title.len() + 4);
    lines.push(format!(
        "\u{2500}\u{2500} {} {}",
        title,
        "\u{2500}".repeat(bar_len)
    ));

    if step_tokens > 0 {
        lines.push(format_row("step", step_tokens, ""));
    }

    if direction_tokens > 0 {
        let dir_detail = if breakdown.direction_names.is_empty() {
            "\u{2014}".to_string()
        } else {
            breakdown.direction_names.join(", ")
        };
        lines.push(format_row("direction", direction_tokens, &dir_detail));
    }

    if breakdown.system_tokens > 0 {
        lines.push(format_row("system", breakdown.system_tokens, "loopflow"));
    }

    if scratch_tokens > 0 {
        lines.push(format_row(
            "scratch",
            scratch_tokens,
            &format!("{} files", breakdown.source_count(DocumentSource::Scratch)),
        ));
    }

    if wave_tokens > 0 {
        lines.push(format_row(
            "wave",
            wave_tokens,
            &format!("{} files", wave_file_count),
        ));
    }

    if diff_tokens > 0 {
        let diff_detail = match breakdown.diff_tier {
            DiffTier::UnifiedDiff => format!("unified ({} files)", breakdown.diff_file_count),
            DiffTier::StatOnly => format!("stat ({} files)", breakdown.diff_file_count),
            DiffTier::None => "\u{2014}".to_string(),
        };
        lines.push(format_row("diff", diff_tokens, &diff_detail));
    }

    if docs_tokens > 0 {
        let docs_detail = format!("{} files", breakdown.source_count(DocumentSource::RepoDoc));
        lines.push(format_row("docs", docs_tokens, &docs_detail));
    }

    if wave_memory_tokens > 0 {
        lines.push(format_row("memory", wave_memory_tokens, "wave"));
    }

    // Area
    let area_doc_count = breakdown.source_count(DocumentSource::Area);
    if area_tokens > 0 && area_doc_count > 0 {
        let area_detail = match &breakdown.area_name {
            Some(name) => format!("{} ({} files)", name, area_doc_count),
            None => format!("{} files", area_doc_count),
        };
        lines.push(format_row("area", area_tokens, &area_detail));
    }

    // Wave
    if let Some(ref wave) = breakdown.wave_name {
        lines.push(format_row("scope", 0, wave));
    }

    // Clipboard
    if breakdown.has_clipboard && clipboard_tokens > 0 {
        lines.push(format_row("clipboard", clipboard_tokens, ""));
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
            source_tokens: std::collections::HashMap::from([
                (DocumentSource::Step, 1000),
                (DocumentSource::Direction, 500),
                (DocumentSource::Diff, 5000),
                (DocumentSource::RepoDoc, 2000),
            ]),
            system_tokens: 3000,
            step_name: Some("implement".to_string()),
            direction_names: vec!["security".to_string()],
            diff_tier: DiffTier::UnifiedDiff,
            diff_file_count: 8,
            source_counts: std::collections::HashMap::from([(DocumentSource::RepoDoc, 1)]),
            ..Default::default()
        };
        let header = format_context_header(&breakdown, 75_000);
        assert!(header.contains("\u{2500}\u{2500} implement \u{2500}"));
        assert!(header.contains("security"));
        assert!(header.contains("unified (8 files)"));
        assert!(header.contains("1 files"));
        assert!(header.contains("15% of 75k"));
    }

    #[test]
    fn format_context_header_splits_docs_rows() {
        let breakdown = ContextBreakdown {
            source_tokens: std::collections::HashMap::from([
                (DocumentSource::Scratch, 600),
                (DocumentSource::Wave, 700),
                (DocumentSource::Summary, 100),
                (DocumentSource::RepoDoc, 500),
            ]),
            source_counts: std::collections::HashMap::from([
                (DocumentSource::Scratch, 2),
                (DocumentSource::Wave, 3),
                (DocumentSource::RepoDoc, 1),
            ]),
            ..Default::default()
        };

        let header = format_context_header(&breakdown, 75_000);
        assert!(header.contains("scratch"));
        assert!(header.contains("wave"));
        assert!(header.contains("docs"));
        assert!(header.contains("2 files"));
        assert!(header.contains("3 files"));
        assert!(header.contains("1 files"));
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
            &["security".to_string()],
            Some("rust"),
            Some("src/"),
            true,
            Some("claude:opus"),
        );
        assert_eq!(
            cmd,
            "lf implement -d security --wave rust -a src/ -c -m claude:opus"
        );
    }
}
