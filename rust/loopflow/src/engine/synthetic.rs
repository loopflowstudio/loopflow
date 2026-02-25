use serde_json::json;

/// A synthetic tool the engine injects into agent context.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticTool {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
    pub guidance: String,
}

/// Execution context hints supplied by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientContext {
    pub has_ui: bool,
    pub compact: bool,
}

/// Synthetic tools available for this launch context.
pub fn synthetic_tools_for_context(
    ctx: &ClientContext,
    action_style: Option<&str>,
) -> Vec<SyntheticTool> {
    if ctx.has_ui {
        vec![suggest_actions_tool(ctx, action_style)]
    } else {
        Vec::new()
    }
}

/// Render synthetic tool guidance for provider system prompts.
pub fn render_synthetic_guidance(tools: &[SyntheticTool]) -> String {
    if tools.is_empty() {
        return String::new();
    }

    let body = tools
        .iter()
        .map(render_tool_guidance)
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("<lf:synthetic_tools>\n{body}\n</lf:synthetic_tools>")
}

fn suggest_actions_tool(ctx: &ClientContext, action_style: Option<&str>) -> SyntheticTool {
    let max_actions = if ctx.compact { 3 } else { 4 };
    let style_guidance = match action_style {
        Some("procedural") => {
            "Suggest actions that move the workflow forward. Prefer clear next steps and binary choices."
        }
        Some("exploratory") => {
            "Suggest actions that open meaningful branches. Prefer options that explore alternatives or deepen understanding."
        }
        _ => {
            "Prefer concrete actions (\"Land PR\", \"Run tests\") over vague actions (\"Continue\", \"Tell me more\")."
        }
    };

    SyntheticTool {
        name: "suggest_actions".to_string(),
        description: "Suggest next actions the user might want to take.".to_string(),
        schema: json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "label": {"type": "string"},
                    "description": {"type": "string"}
                },
                "required": ["label"]
            }
        }),
        guidance: format!(
            "Use suggest_actions to suggest up to {max_actions} next actions the user might want to take. \
Call it after completing a task, when waiting for user input, or when presenting results. \
Each action should be a short phrase that makes sense as a user message. {style_guidance}"
        ),
    }
}

fn render_tool_guidance(tool: &SyntheticTool) -> String {
    format!(
        "Tool: {name}\nDescription: {description}\nGuidance: {guidance}\n\
When you call this tool, emit exactly this tagged JSON payload format:\n\
<lf:{name}>\n[{{\"label\":\"Land PR\",\"description\":\"Merge and clean up\"}},{{\"label\":\"Run tests\"}}]\n</lf:{name}>\n\
Inside the tag, output valid JSON only.",
        name = tool.name,
        description = tool.description,
        guidance = tool.guidance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_tools_for_context_requires_ui() {
        let tools = synthetic_tools_for_context(
            &ClientContext {
                has_ui: false,
                compact: false,
            },
            None,
        );
        assert!(tools.is_empty());
    }

    #[test]
    fn synthetic_tools_for_context_adds_suggest_actions_for_ui() {
        let tools = synthetic_tools_for_context(
            &ClientContext {
                has_ui: true,
                compact: false,
            },
            None,
        );

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "suggest_actions");
        assert!(tools[0].guidance.contains("up to 4 next actions"));
    }

    #[test]
    fn synthetic_tools_for_context_uses_compact_limit() {
        let tools = synthetic_tools_for_context(
            &ClientContext {
                has_ui: true,
                compact: true,
            },
            None,
        );

        assert_eq!(tools.len(), 1);
        assert!(tools[0].guidance.contains("up to 3 next actions"));
    }

    #[test]
    fn synthetic_tools_for_context_applies_action_style_guidance() {
        let procedural = synthetic_tools_for_context(
            &ClientContext {
                has_ui: true,
                compact: false,
            },
            Some("procedural"),
        );
        assert!(procedural[0].guidance.contains("workflow forward"));

        let exploratory = synthetic_tools_for_context(
            &ClientContext {
                has_ui: true,
                compact: false,
            },
            Some("exploratory"),
        );
        assert!(exploratory[0].guidance.contains("meaningful branches"));
    }

    #[test]
    fn render_synthetic_guidance_wraps_tools_block() {
        let tools = synthetic_tools_for_context(
            &ClientContext {
                has_ui: true,
                compact: false,
            },
            None,
        );
        let rendered = render_synthetic_guidance(&tools);
        assert!(rendered.contains("<lf:synthetic_tools>"));
        assert!(rendered.contains("<lf:suggest_actions>"));
    }
}
