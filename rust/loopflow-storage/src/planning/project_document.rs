//! A Linear Project's Markdown `content`: its description, and the `## KRs`
//! and `## Metric targets` sections released `lf` wrote there. Nothing writes
//! those sections now; they are kept out of the description and left in place
//! when the description changes. Released stores convert them once, on upgrade.

use super::ProjectDocumentError;
use serde::{Deserialize, Serialize};

type ProjectDocumentResult<T> = Result<T, ProjectDocumentError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Krs,
    Targets,
}

/// A KR as released `lf` stored it in `projects.krs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasedKr {
    pub text: String,
    pub holds: bool,
}

/// A Linear Markdown boundary, not a stored Project or another planning owner.
#[derive(Debug)]
pub struct ProjectDocument<'a> {
    /// The `## Metric targets` JSON array, unread except by released conversion.
    pub released_targets: Vec<serde_json::Value>,
    pub released_krs: Vec<ReleasedKr>,
    source: &'a str,
    fields: Vec<std::ops::Range<usize>>,
}

impl ProjectDocument<'_> {
    pub fn description(&self) -> String {
        let mut prose = String::new();
        let mut start = 0;
        for range in &self.fields {
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
        for range in &self.fields {
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
    let mut current_kr: Option<ReleasedKr> = None;
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
            fields.push(range);
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
                    if let Some(kr) = current_kr.take() {
                        krs.push(kr);
                    }
                    if !text.trim().is_empty() {
                        current_kr = Some(ReleasedKr {
                            text: text.trim().into(),
                            holds,
                        });
                        fields.push(range);
                    }
                } else if trimmed.is_empty() {
                    // A new paragraph is prose, not a continuation of the last KR.
                    if let Some(kr) = current_kr.take() {
                        krs.push(kr);
                    }
                } else if let Some(kr) = current_kr.as_mut() {
                    kr.text.push(' ');
                    kr.text.push_str(trimmed);
                    fields.push(range);
                }
            }
            _ => {}
        }
    }
    if let Some(kr) = current_kr {
        krs.push(kr);
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
    fn released_sections_stay_out_of_the_description_and_survive_its_edit() {
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
        let edited = document.replace_description("New prose").unwrap();
        assert!(edited.starts_with("New prose\n\n"));
        for kept in [
            "workflow: code\n",
            "## KRs\n",
            "- [x] One proof holds",
            "\"rate\"",
        ] {
            assert!(edited.contains(kept), "{edited}");
        }
    }

    #[test]
    fn unreadable_targets_are_refused() {
        assert!(parse_project_document("## Metric targets\nnot JSON").is_err());
        assert!(parse_project_document("## Metric targets\n```json\n[1\n").is_err());
    }
}
