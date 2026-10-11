//! Linear Markdown and released-content conversion; never an ordinary Work model.

use super::{PmError, PmResult};
use crate::work::project::ProjectKR;
use crate::work::wave::metrics::MetricTarget;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub(crate) struct LinearProjectFields {
    pub metric_targets: Vec<MetricTarget>,
    pub workflow: String,
    pub krs: Vec<ProjectKR>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectField {
    Workflow,
    Krs,
    Targets,
}

/// A Linear Markdown boundary, not a stored Project or another planning owner.
#[derive(Debug)]
pub(crate) struct ProjectDocument<'a> {
    pub content: LinearProjectFields,
    source: &'a str,
    fields: Vec<(std::ops::Range<usize>, ProjectField)>,
}

impl ProjectDocument<'_> {
    pub fn rewrite(&self, field: Option<ProjectField>, replacement: &str) -> String {
        let mut result = String::new();
        let mut start = 0;
        let mut pending = Some(replacement);
        for (range, owner) in &self.fields {
            if field.is_none_or(|field| field == *owner) {
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

pub fn parse_project_content(content: &str) -> PmResult<LinearProjectFields> {
    Ok(parse_project_document(content)?.content)
}

pub(crate) fn parse_project_document(source: &str) -> PmResult<ProjectDocument<'_>> {
    use ProjectField::{Krs, Targets, Workflow};
    let mut section = None;
    let mut fields = Vec::new();
    let mut fence: Option<(char, usize, bool)> = None;
    let mut fenced_targets = false;
    let mut targets = Vec::new();
    let mut raw_targets = Vec::new();
    let mut workflow = String::new();
    let mut authored_as_flow = String::new();
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
        if let Some(value) = trimmed.strip_prefix("workflow:") {
            workflow = value.trim().to_string();
            fields.push((range, Workflow));
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("flow:") {
            authored_as_flow = value.trim().to_string();
            fields.push((range, Workflow));
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
        return Err(PmError::Message(
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
        serde_json::from_str(&targets.concat())
            .map_err(|error| PmError::Message(format!("invalid Project metric targets: {error}")))?
    };
    if workflow.is_empty() {
        workflow = authored_as_flow;
    }
    let content = LinearProjectFields {
        metric_targets,
        workflow,
        krs,
    };
    content.validate()?;
    fields.sort_unstable_by_key(|(range, _)| range.start);
    Ok(ProjectDocument {
        content,
        source,
        fields,
    })
}

impl LinearProjectFields {
    pub fn validate(&self) -> PmResult<()> {
        crate::work::project::validate_metric_targets(&self.metric_targets)
            .map_err(PmError::Message)
    }
}

pub(crate) fn render_project_content(
    workflow: &str,
    krs: &[ProjectKR],
    metric_targets: &[MetricTarget],
) -> String {
    let mut content = render_metric_targets(metric_targets);
    let workflow = workflow.trim();
    if !workflow.is_empty() {
        content.push_str(&format!("\n\nworkflow: {workflow}"));
    }
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
    use super::{parse_project_content, render_project_content, LinearProjectFields};
    use crate::work::project::ProjectKR;
    use crate::work::wave::metrics::MetricTarget;

    #[test]
    fn project_content_without_workflow_preserves_krs() {
        let content = parse_project_content("## KRs\n- [ ] Keep this proof\n").unwrap();
        assert!(content.workflow.is_empty());
        assert_eq!(content.krs[0].text, "Keep this proof");
        let rendered =
            render_project_content(&content.workflow, &content.krs, &content.metric_targets);
        assert!(!rendered.contains("flow:"));
        assert_eq!(parse_project_content(&rendered).unwrap(), content);
    }

    #[test]
    fn a_project_written_with_a_flow_line_names_that_workflow_until_rewritten() {
        let earlier = parse_project_content("flow: research\n\n## KRs\n").unwrap();
        assert_eq!(earlier.workflow, "research");
        assert!(
            render_project_content(&earlier.workflow, &earlier.krs, &earlier.metric_targets)
                .contains("\n\nworkflow: research\n")
        );
        let both = parse_project_content("flow: research\nworkflow: code\n").unwrap();
        assert_eq!(both.workflow, "code");
    }

    #[test]
    fn project_content_round_trips_linear_markdown() {
        let project = LinearProjectFields {
            metric_targets: vec![MetricTarget {
                metric_id: "throughput".into(),
                target: crate::work::wave::metrics::Threshold::AtLeast { value: 0.95 },
            }],
            workflow: "feature".to_string(),
            krs: vec![
                ProjectKR {
                    text: "One proof holds".to_string(),
                    holds: true,
                },
                ProjectKR {
                    text: "Another remains".to_string(),
                    holds: false,
                },
            ],
        };
        let rendered =
            render_project_content(&project.workflow, &project.krs, &project.metric_targets);
        assert_eq!(parse_project_content(&rendered).unwrap(), project);

        let local = "# Project Name\n\nA measured bet.\n\n## KRs\n\n- One proof holds\n  across wrapped lines.\n";
        assert_eq!(
            parse_project_content(local).unwrap(),
            LinearProjectFields {
                metric_targets: Vec::new(),
                workflow: String::new(),
                krs: vec![ProjectKR {
                    text: "One proof holds across wrapped lines.".to_string(),
                    holds: false,
                }],
            }
        );
    }
}

#[cfg(test)]
mod chapter_content_tests {
    use super::parse_project_content;
    #[test]
    fn project_markdown_rejects_ambiguous_targets() {
        let duplicate = r##"## Metric targets
```json
[{"metric_id":"rate","target":{"kind":"at_least","value":0.9}},
 {"metric_id":"rate","target":{"kind":"at_most","value":0.1}}]
```
"##;
        assert!(parse_project_content(duplicate).is_err());
        assert!(parse_project_content("## Metric targets\nnot JSON").is_err());
    }
}
