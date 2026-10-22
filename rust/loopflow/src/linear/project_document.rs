//! Linear Markdown and released-content conversion; never an ordinary Work model.

use super::{LinearError, LinearResult};
use crate::work::project::ProjectKR;
use crate::work::wave::metrics::MetricTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectField {
    HistoricalWorkflow,
    Krs,
    Targets,
}

/// A Linear Markdown boundary, not a stored Project or another planning owner.
#[derive(Debug)]
pub(crate) struct ProjectDocument<'a> {
    pub metric_targets: Vec<MetricTarget>,
    pub krs: Vec<ProjectKR>,
    source: &'a str,
    fields: Vec<(std::ops::Range<usize>, ProjectField)>,
}

impl ProjectDocument<'_> {
    pub fn description(&self) -> String {
        let mut prose = String::new();
        let mut start = 0;
        for (range, _) in &self.fields {
            prose.push_str(&self.source[start..range.start]);
            start = range.end;
        }
        prose.push_str(&self.source[start..]);
        prose
    }

    /// Managed spans own their separating whitespace, not the prose value.
    /// Released-content conversion deliberately keeps using the lossless reader.
    pub fn observed_description(&self) -> String {
        let mut parts = Vec::new();
        let mut start = 0;
        for (range, _) in &self.fields {
            let part = self.source[start..range.start].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = range.end;
        }
        let part = self.source[start..].trim();
        if !part.is_empty() {
            parts.push(part);
        }
        parts.join("\n\n")
    }

    pub fn replace_description(&self, description: &str) -> LinearResult<String> {
        // Prose and managed fields share Markdown syntax. Fenced examples are
        // prose; an unclosed fence or live managed declaration cannot silently
        // change the independently saved fields.
        let mut result = description.trim_end().to_owned();
        result.push_str("\n\n");
        for (range, _) in &self.fields {
            result.push_str(&self.source[range.clone()]);
        }
        let parsed = parse_project_document(&result)?;
        if parsed.krs != self.krs
            || parsed.metric_targets != self.metric_targets
            || parsed.description().trim() != description.trim()
        {
            return Err(LinearError::Message(
                "Project prose changes managed fields; put Markdown examples in closed fences"
                    .into(),
            ));
        }
        Ok(result)
    }

    pub fn rewrite(&self, field: ProjectField, replacement: &str) -> String {
        let mut result = String::new();
        let mut start = 0;
        let mut pending = Some(replacement);
        for (range, owner) in &self.fields {
            if field == *owner {
                result.push_str(&self.source[start..range.start]);
                if let Some(replacement) = pending.take() {
                    result.push_str(replacement);
                }
                start = range.end;
            }
        }
        result.push_str(&self.source[start..]);
        if let Some(replacement) = pending.filter(|replacement| !replacement.is_empty()) {
            result.push_str("\n\n");
            result.push_str(replacement);
        }
        result
    }
}

pub(crate) fn parse_project_document(source: &str) -> LinearResult<ProjectDocument<'_>> {
    use ProjectField::{HistoricalWorkflow, Krs, Targets};
    let mut section = None;
    let mut fields = Vec::new();
    let mut fence: Option<(char, usize, bool)> = None;
    let mut fenced_targets = false;
    let mut targets = Vec::new();
    let mut raw_targets = Vec::new();
    let mut krs = Vec::new();
    let mut current_kr: Option<ProjectKR> = None;
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let range = offset..offset + line.len();
        offset = range.end;
        let trimmed = line.trim();
        if let Some((marker, length, managed)) = fence {
            let closing = trimmed.chars().take_while(|ch| *ch == marker).count() >= length
                && trimmed.trim_matches(marker).is_empty();
            if managed {
                fields.push((range, Targets));
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
                    fields.push((range, Targets));
                }
                if let Some(kr) = current_kr.take() {
                    krs.push(kr);
                }
                fence = Some((marker, length, managed));
                continue;
            }
        }
        // Retain released declarations byte-for-byte when editing other fields;
        // they no longer select a Workflow or need value/precedence parsing.
        if trimmed.starts_with("workflow:") || trimmed.starts_with("flow:") {
            fields.push((range, HistoricalWorkflow));
            continue;
        }
        if trimmed.starts_with('#') {
            if let Some(kr) = current_kr.take() {
                krs.push(kr);
            }
            section = match trimmed {
                "## Metric targets" => Some(Targets),
                "## KRs" => Some(Krs),
                _ => None,
            };
            if let Some(section) = section {
                fields.push((range, section));
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
                    if let Some(kr) = current_kr.take() {
                        krs.push(kr);
                    }
                    if !text.trim().is_empty() {
                        current_kr = Some(ProjectKR {
                            text: text.trim().into(),
                            holds,
                        });
                        fields.push((range, Krs));
                    }
                } else if trimmed.is_empty() {
                    // A new paragraph is prose, not a continuation of the last KR.
                    if let Some(kr) = current_kr.take() {
                        krs.push(kr);
                    }
                } else if let Some(kr) = current_kr.as_mut() {
                    kr.text.push(' ');
                    kr.text.push_str(trimmed);
                    fields.push((range, Krs));
                }
            }
            _ => {}
        }
    }
    if let Some(kr) = current_kr {
        krs.push(kr);
    }
    if fence.is_some_and(|(_, _, managed)| managed) {
        return Err(LinearError::Message(
            "unterminated Project metric targets fence".into(),
        ));
    }
    if !fenced_targets {
        for (line, range) in raw_targets {
            targets.push(line);
            if !line.trim().is_empty() {
                fields.push((range, Targets));
            }
        }
    }

    let metric_targets = if targets.iter().all(|line| line.trim().is_empty()) {
        Vec::new()
    } else {
        serde_json::from_str(&targets.concat()).map_err(|error| {
            LinearError::Message(format!("invalid Project metric targets: {error}"))
        })?
    };
    crate::work::project::validate_metric_targets(&metric_targets).map_err(LinearError::Message)?;
    fields.sort_unstable_by_key(|(range, _)| range.start);
    Ok(ProjectDocument {
        metric_targets,
        krs,
        source,
        fields,
    })
}

pub(crate) fn render_project_content(krs: &[ProjectKR], metric_targets: &[MetricTarget]) -> String {
    let mut content = render_metric_targets(metric_targets);
    content.push_str("\n\n");
    content.push_str(&render_krs(krs));
    content.push('\n');
    content
}

pub(super) fn render_metric_targets(metric_targets: &[MetricTarget]) -> String {
    format!(
        "## Metric targets\n\n```json\n{}\n```",
        serde_json::to_string_pretty(metric_targets).expect("validated metric targets serialize")
    )
}

pub(super) fn render_krs(krs: &[ProjectKR]) -> String {
    let mut content = String::from("## KRs");
    for kr in krs {
        let marker = if kr.holds { "x" } else { " " };
        content.push_str(&format!("\n\n- [{marker}] {}", kr.text.trim()));
    }
    content
}

#[cfg(test)]
mod tests {
    use super::{parse_project_document, render_project_content};
    use crate::work::project::ProjectKR;
    use crate::work::wave::metrics::MetricTarget;

    #[test]
    fn project_content_without_workflow_preserves_krs() {
        let content = parse_project_document("## KRs\n- [ ] Keep this proof\n").unwrap();
        assert_eq!(content.krs[0].text, "Keep this proof");
        let rendered = render_project_content(&content.krs, &content.metric_targets);
        assert!(!rendered.contains("flow:"));
        let reparsed = parse_project_document(&rendered).unwrap();
        assert_eq!(reparsed.krs, content.krs);
        assert_eq!(reparsed.metric_targets, content.metric_targets);
    }

    #[test]
    fn released_workflow_content_is_readable_but_never_republished() {
        let source = "flow: research\nworkflow: code\n\n## KRs\n";
        let document = parse_project_document(source).unwrap();
        assert!(!render_project_content(&document.krs, &document.metric_targets).contains("flow:"));
        let edited = document.replace_description("New prose").unwrap();
        assert!(edited.contains("flow: research\nworkflow: code\n"));
    }

    #[test]
    fn project_content_round_trips_linear_markdown() {
        let targets = vec![MetricTarget {
            metric_id: "throughput".into(),
            target: crate::work::wave::metrics::Threshold::AtLeast { value: 0.95 },
        }];
        let krs = vec![
            ProjectKR {
                text: "One proof holds".into(),
                holds: true,
            },
            ProjectKR {
                text: "Another remains".into(),
                holds: false,
            },
        ];
        let rendered = render_project_content(&krs, &targets);
        let parsed = parse_project_document(&rendered).unwrap();
        assert_eq!(parsed.krs, krs);
        assert_eq!(parsed.metric_targets, targets);

        let local = "# Project Name\n\nA measured bet.\n\n## KRs\n\n- One proof holds\n  across wrapped lines.\n";
        let parsed = parse_project_document(local).unwrap();
        assert!(parsed.metric_targets.is_empty());
        assert_eq!(
            parsed.krs,
            vec![ProjectKR {
                text: "One proof holds across wrapped lines.".into(),
                holds: false,
            }]
        );
    }

    #[test]
    fn project_markdown_rejects_ambiguous_targets() {
        let duplicate = r##"## Metric targets
```json
[{"metric_id":"rate","target":{"kind":"at_least","value":0.9}},
 {"metric_id":"rate","target":{"kind":"at_most","value":0.1}}]
```
"##;
        assert!(parse_project_document(duplicate).is_err());
        assert!(parse_project_document("## Metric targets\nnot JSON").is_err());
    }
}
