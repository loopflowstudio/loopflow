/// Structured data the model emits embedded in its reply text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredReply {
    pub name: String,
    pub description: String,
    pub guidance: String,
}

/// Execution context hints supplied by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientContext {
    pub has_ui: bool,
    pub compact: bool,
}

/// Structured replies available for this launch context.
pub fn structured_replies_for_context(
    ctx: &ClientContext,
    action_style: Option<&str>,
) -> Vec<StructuredReply> {
    if ctx.has_ui {
        vec![suggest_actions_reply(ctx, action_style)]
    } else {
        Vec::new()
    }
}

/// Render structured reply guidance for provider system prompts.
pub fn render_structured_reply_guidance(replies: &[StructuredReply]) -> String {
    if replies.is_empty() {
        return String::new();
    }

    let body = replies
        .iter()
        .map(render_reply_guidance)
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("<lf:structured_replies>\n{body}\n</lf:structured_replies>")
}

fn suggest_actions_reply(ctx: &ClientContext, action_style: Option<&str>) -> StructuredReply {
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

    StructuredReply {
        name: "suggest_actions".to_string(),
        description: "Suggest next actions the user might want to take.".to_string(),
        guidance: format!(
            "REQUIRED: After every response where you complete a task, answer a question, or present results, \
you MUST emit a suggest_actions block at the end of your reply. Never skip this. \
The user's client renders these as tap targets — without them, the user has to type on a phone keyboard. \
Suggest up to {max_actions} actions. Each action should be a short phrase that makes sense as a user message. \
{style_guidance}"
        ),
    }
}

fn render_reply_guidance(reply: &StructuredReply) -> String {
    format!(
        "Tool: {name}\nDescription: {description}\nGuidance: {guidance}\n\
When you call this tool, emit exactly this tagged JSON payload format:\n\
<lf:{name}>\n[{{\"label\":\"Land PR\",\"description\":\"Merge and clean up\"}},{{\"label\":\"Run tests\"}}]\n</lf:{name}>\n\
Inside the tag, output valid JSON only.",
        name = reply.name,
        description = reply.description,
        guidance = reply.guidance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_replies_for_context_requires_ui() {
        let replies = structured_replies_for_context(
            &ClientContext {
                has_ui: false,
                compact: false,
            },
            None,
        );
        assert!(replies.is_empty());
    }

    #[test]
    fn structured_replies_for_context_adds_suggest_actions_for_ui() {
        let replies = structured_replies_for_context(
            &ClientContext {
                has_ui: true,
                compact: false,
            },
            None,
        );

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].name, "suggest_actions");
        assert!(replies[0].guidance.contains("up to 4"));
    }

    #[test]
    fn structured_replies_for_context_uses_compact_limit() {
        let replies = structured_replies_for_context(
            &ClientContext {
                has_ui: true,
                compact: true,
            },
            None,
        );

        assert_eq!(replies.len(), 1);
        assert!(replies[0].guidance.contains("up to 3"));
    }

    #[test]
    fn structured_replies_for_context_applies_action_style_guidance() {
        let procedural = structured_replies_for_context(
            &ClientContext {
                has_ui: true,
                compact: false,
            },
            Some("procedural"),
        );
        assert!(procedural[0].guidance.contains("workflow forward"));

        let exploratory = structured_replies_for_context(
            &ClientContext {
                has_ui: true,
                compact: false,
            },
            Some("exploratory"),
        );
        assert!(exploratory[0].guidance.contains("meaningful branches"));
    }

    #[test]
    fn render_structured_reply_guidance_wraps_block() {
        let replies = structured_replies_for_context(
            &ClientContext {
                has_ui: true,
                compact: false,
            },
            None,
        );
        let rendered = render_structured_reply_guidance(&replies);
        assert!(rendered.contains("<lf:structured_replies>"));
        assert!(rendered.contains("<lf:suggest_actions>"));
    }
}
