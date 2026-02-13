use crate::agent::anthropic::{ContentBlock, ToolDefinition};

/// Dispatch a tool call by name. Returns the string result.
pub fn dispatch(name: &str, input: &serde_json::Value) -> String {
    match name {
        "get_current_time" => tool_get_current_time(),
        "calculate" => tool_calculate(input),
        _ => format!("unknown tool: {name}"),
    }
}

/// Return tool definitions for all registered tools.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "get_current_time".to_string(),
            description: "Get the current date and time in UTC.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "calculate".to_string(),
            description: "Evaluate a simple arithmetic expression. Supports +, -, *, / with integers and floats.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The arithmetic expression to evaluate, e.g. '2 + 3 * 4'"
                    }
                },
                "required": ["expression"]
            }),
        },
    ]
}

/// Build tool result content blocks from dispatched results.
pub fn make_tool_results(assistant_content: &[ContentBlock]) -> Vec<ContentBlock> {
    assistant_content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => Some(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: dispatch(name, input),
            }),
            _ => None,
        })
        .collect()
}

fn tool_get_current_time() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn tool_calculate(input: &serde_json::Value) -> String {
    let expr = match input.get("expression").and_then(|v| v.as_str()) {
        Some(e) => e,
        None => return "error: missing 'expression' field".to_string(),
    };
    // Simple tokenized arithmetic: supports +, -, *, / with operator precedence
    match eval_expr(expr) {
        Ok(result) => format!("{result}"),
        Err(e) => format!("error: {e}"),
    }
}

fn eval_expr(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    parse_expression(&tokens, &mut 0)
}

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Op(char),
}

fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_digit() || c == '.' {
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() || d == '.' {
                    num.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Token::Num(num.parse::<f64>().map_err(|e| e.to_string())?));
        } else if "+-*/".contains(c) {
            tokens.push(Token::Op(c));
            chars.next();
        } else {
            return Err(format!("unexpected character: {c}"));
        }
    }
    Ok(tokens)
}

// Recursive descent: expression = term ((+|-) term)*
fn parse_expression(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('+') => {
                *pos += 1;
                left += parse_term(tokens, pos)?;
            }
            Token::Op('-') => {
                *pos += 1;
                left -= parse_term(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

// term = factor ((*|/) factor)*
fn parse_term(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_factor(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('*') => {
                *pos += 1;
                left *= parse_factor(tokens, pos)?;
            }
            Token::Op('/') => {
                *pos += 1;
                let right = parse_factor(tokens, pos)?;
                if right == 0.0 {
                    return Err("division by zero".to_string());
                }
                left /= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_factor(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of expression".to_string());
    }
    match &tokens[*pos] {
        Token::Num(n) => {
            let val = *n;
            *pos += 1;
            Ok(val)
        }
        _ => Err(format!("expected number, got {:?}", tokens[*pos])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_simple() {
        let input = serde_json::json!({"expression": "2 + 3"});
        assert_eq!(tool_calculate(&input), "5");
    }

    #[test]
    fn test_calculate_precedence() {
        let input = serde_json::json!({"expression": "2 + 3 * 4"});
        assert_eq!(tool_calculate(&input), "14");
    }

    #[test]
    fn test_calculate_division() {
        let result: f64 = tool_calculate(&serde_json::json!({"expression": "10 / 3"}))
            .parse()
            .unwrap();
        assert!((result - 3.3333333333333335).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_division_by_zero() {
        let input = serde_json::json!({"expression": "5 / 0"});
        assert!(tool_calculate(&input).contains("division by zero"));
    }

    #[test]
    fn test_dispatch_unknown_tool() {
        let result = dispatch("nonexistent", &serde_json::json!({}));
        assert!(result.contains("unknown tool"));
    }

    #[test]
    fn test_get_current_time_is_rfc3339() {
        let result = tool_get_current_time();
        // Should be parseable as a datetime
        assert!(result.contains('T'));
        assert!(result.ends_with("+00:00") || result.ends_with('Z'));
    }
}
