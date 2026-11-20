//! A Linear Project's Markdown `content`: its description, and the `## KRs`
//! and `## Metric targets` sections released `lf` wrote there. Nothing writes
//! those sections now; they are kept out of the description and left in place
//! when the description changes. Released stores convert them once, on upgrade.

use super::ProjectDocumentError;
use serde::Serialize;

type ProjectDocumentResult<T> = Result<T, ProjectDocumentError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Krs,
    Targets,
}

/// A KR as released `lf` stored it in `projects.krs`.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ReleasedKr {
    text: String,
    holds: bool,
}

/// A Linear Markdown boundary, not a stored Project or another planning owner.
#[derive(Debug)]
pub struct ProjectDocument<'a> {
    /// The `## Metric targets` JSON array, unread except by released conversion.
    pub(crate) released_targets: Vec<serde_json::Value>,
    pub(crate) released_krs: Vec<ReleasedKr>,
    source: &'a str,
    fields: Vec<std::ops::Range<usize>>,
}

impl ProjectDocument<'_> {
    fn prose_parts(&self) -> impl Iterator<Item = &str> {
        let end = self.source.len();
        let mut start = 0;
        self.fields
            .iter()
            .cloned()
            .chain(std::iter::once(end..end))
            .map(move |field| {
                let prose = &self.source[start..field.start];
                start = field.end;
                prose
            })
    }

    /// Released-store conversion retains the whitespace between prose spans.
    pub(crate) fn description(&self) -> String {
        self.prose_parts().collect()
    }

    pub fn observed_description(&self) -> String {
        self.prose_parts()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn replace_description(&self, description: &str) -> ProjectDocumentResult<String> {
        // Prose and managed fields share Markdown syntax. Fenced examples are
        // prose; an unclosed fence or live managed declaration cannot silently
        // change the independently saved fields.
        let mut result = description.trim_end().to_owned();
        result.push_str("\n\n");
        for range in &self.fields {
            result.push_str(&self.source[range.clone()]);
        }
        let parsed = parse_project_document(&result)?;
        if parsed.released_krs != self.released_krs
            || parsed.released_targets != self.released_targets
            || parsed.description().trim() != description.trim()
        {
            return Err(ProjectDocumentError(
                "Project prose changes managed fields; put Markdown examples in closed fences"
                    .into(),
            ));
        }
        Ok(result)
    }
}

pub fn parse_project_document(source: &str) -> ProjectDocumentResult<ProjectDocument<'_>> {
    use Section::{Krs, Targets};
    let mut section = None;
    let mut fields = Vec::new();
    let mut fence: Option<(char, usize, bool)> = None;
    let mut fenced_targets = false;
    let mut targets = Vec::new();
    let mut raw_targets = Vec::new();
    let mut krs = Vec::new();
    let mut current_kr: Option<&mut ReleasedKr> = None;
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let range = offset..offset + line.len();
        offset = range.end;
        let trimmed = line.trim();
        if let Some((marker, length, managed)) = fence {
            let closing = trimmed.chars().take_while(|ch| *ch == marker).count() >= length
                && trimmed.trim_matches(marker).is_empty();
            if managed {
                fields.push(range);
                if !closing {
                    targets.push(line);
                }
            }
            if closing {
                fence = None;
            }
            continue;
        }
        if let Some(marker @ ('`' | '~')) = trimmed.chars().next() {
            let length = trimmed.chars().take_while(|ch| *ch == marker).count();
            if length >= 3 {
                let language = trimmed[length..].trim();
                let managed = section == Some(Targets) && matches!(language, "" | "json");
                if managed {
                    fenced_targets = true;
                    fields.push(range);
                }
                current_kr = None;
                fence = Some((marker, length, managed));
                continue;
            }
        }
        // Retain released declarations byte-for-byte when editing other fields;
        // they no longer select a Workflow or need value/precedence parsing.
        if trimmed.starts_with("workflow:") || trimmed.starts_with("flow:") {
            fields.push(range);
            continue;
        }
        if trimmed.starts_with('#') {
            current_kr = None;
            section = match trimmed {
                "## Metric targets" => Some(Targets),
                "## KRs" => Some(Krs),
                _ => None,
            };
            if section.is_some() {
                fields.push(range);
            }
            continue;
        }
        match section {
            Some(Targets) => raw_targets.push((line, range)),
            Some(Krs) => {
                let item = if let Some(text) = trimmed.strip_prefix("- [x] ") {
                    Some((true, text))
                } else if let Some(text) = trimmed.strip_prefix("- [X] ") {
                    Some((true, text))
                } else if let Some(text) = trimmed.strip_prefix("- [ ] ") {
                    Some((false, text))
                } else {
                    trimmed.strip_prefix("- ").map(|text| (false, text))
                };
                if let Some((holds, text)) = item {
                    krs.push(ReleasedKr {
                        text: text.trim().into(),
                        holds,
                    });
                    current_kr = krs.last_mut();
                    fields.push(range);
                } else if trimmed.is_empty() {
                    // A new paragraph is prose, not a continuation of the last KR.
                    current_kr = None;
                } else if let Some(kr) = current_kr.as_mut() {
                    kr.text.push(' ');
                    kr.text.push_str(trimmed);
                    fields.push(range);
                }
            }
            _ => {}
        }
    }
    if fence.is_some_and(|(_, _, managed)| managed) {
        return Err(ProjectDocumentError(
            "unterminated Project metric targets fence".into(),
        ));
    }
    if !fenced_targets {
        for (line, range) in raw_targets {
            targets.push(line);
            if !line.trim().is_empty() {
                fields.push(range);
            }
        }
    }

    let released_targets = if targets.iter().all(|line| line.trim().is_empty()) {
        Vec::new()
    } else {
        serde_json::from_str::<Vec<serde_json::Value>>(&targets.concat()).map_err(|error| {
            ProjectDocumentError(format!("invalid Project metric targets: {error}"))
        })?
    };
    fields.sort_unstable_by_key(|range| range.start);
    Ok(ProjectDocument {
        released_targets,
        released_krs: krs,
        source,
        fields,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_project_document, ReleasedKr};

    #[test]
    fn released_sections_are_read_separately_from_prose() {
        let source = "flow: research\nworkflow: code\n\nA measured bet.\n\n## Metric targets\n```json\n[{\"metric_id\":\"rate\"}]\n```\n\n## KRs\n\n- [x] One proof holds\n- Another\n  across wrapped lines.\n";
        let document = parse_project_document(source).unwrap();
        assert_eq!(document.observed_description(), "A measured bet.");
        assert_eq!(
            document.released_krs,
            vec![
                ReleasedKr {
                    text: "One proof holds".into(),
                    holds: true
                },
                ReleasedKr {
                    text: "Another across wrapped lines.".into(),
                    holds: false
                },
            ]
        );
        assert_eq!(document.released_targets[0]["metric_id"], "rate");
    }

    #[test]
    fn unreadable_targets_are_refused() {
        assert!(parse_project_document("## Metric targets\nnot JSON").is_err());
        assert!(parse_project_document("## Metric targets\n```json\n[1\n").is_err());
    }

    #[test]
    fn prose_edits_preserve_released_sections_and_reject_live_declarations() {
        let source = "Old introduction.\nworkflow: code\n## Metric targets\nTarget rationale.\n```json\n[{\"metric_id\":\"proof\",\"target\":{\"kind\":\"at_least\",\"value\":1.0}}]\n```\n## KRs\nPreamble.\n- [ ] Keep this criterion\n  across lines.\n\nOld closing.\n";
        let before = parse_project_document(source).unwrap();
        for description in ["", "New authored prose.", "## Examples\n````markdown\n```yaml\nworkflow: example\n```\n## KRs\n- [x] Example only\n````\nClosing prose.\n"] {
            let after = before.replace_description(description).unwrap();
            let parsed = parse_project_document(&after).unwrap();
            assert_eq!(parsed.description().trim(), description.trim());
            assert!(after.contains("workflow: code\n"));
            assert!(after.contains("- [ ] Keep this criterion\n  across lines.\n"));
            assert_eq!(parsed.released_krs, before.released_krs);
            assert_eq!(parsed.released_targets, before.released_targets);
            assert!(!after.contains("Old introduction"));
            assert!(!after.contains("Target rationale"));
            assert!(!after.contains("Old closing"));
        }
        for description in [
            "workflow: replacement",
            "## KRs\n- [x] Replacement",
            "```markdown\nUnclosed example",
        ] {
            assert!(
                before.replace_description(description).is_err(),
                "{description}"
            );
        }
    }
}
