use crate::agent::anthropic::ToolDefinition;
use crate::agent::registry::{Tool, ToolRegistry, ToolResult};

// --- Tool implementations ---

pub struct GetCurrentTime;

impl Tool for GetCurrentTime {
    fn name(&self) -> &str {
        "get_current_time"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_current_time".to_string(),
            description: "Get the current date and time in UTC.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    fn call(&self, _input: &serde_json::Value) -> ToolResult {
        ToolResult {
            output: chrono::Utc::now().to_rfc3339(),
            event: None,
        }
    }
}

pub struct Calculate;

impl Tool for Calculate {
    fn name(&self) -> &str {
        "calculate"
    }

    fn definition(&self) -> ToolDefinition {
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
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let output = match input.get("expression").and_then(|v| v.as_str()) {
            Some(expr) => match eval_expr(expr) {
                Ok(result) => format!("{result}"),
                Err(e) => format!("error: {e}"),
            },
            None => "error: missing 'expression' field".to_string(),
        };
        ToolResult {
            output,
            event: None,
        }
    }
}

/// Build a registry with the default built-in tools.
pub fn default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(GetCurrentTime));
    registry.register(Box::new(Calculate));
    registry
}

// --- Arithmetic evaluation ---

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
    fn calculate_simple() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({"expression": "2 + 3"}));
        assert_eq!(result.output, "5");
        assert!(result.event.is_none());
    }

    #[test]
    fn calculate_precedence() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({"expression": "2 + 3 * 4"}));
        assert_eq!(result.output, "14");
    }

    #[test]
    fn calculate_division() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({"expression": "10 / 3"}));
        let val: f64 = result.output.parse().unwrap();
        assert!((val - 3.3333333333333335).abs() < 1e-10);
    }

    #[test]
    fn calculate_division_by_zero() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({"expression": "5 / 0"}));
        assert!(result.output.contains("division by zero"));
    }

    #[test]
    fn calculate_missing_expression() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({}));
        assert!(result.output.contains("missing 'expression' field"));
    }

    #[test]
    fn get_current_time_is_rfc3339() {
        let tool = GetCurrentTime;
        let result = tool.call(&serde_json::json!({}));
        assert!(result.output.contains('T'));
        assert!(result.output.ends_with("+00:00") || result.output.ends_with('Z'));
    }

    #[test]
    fn default_registry_has_both_tools() {
        let registry = default_registry();
        let defs = registry.definitions();
        assert_eq!(defs.len(), 2);

        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"get_current_time"));
        assert!(names.contains(&"calculate"));
    }

    #[test]
    fn default_registry_dispatches_calculate() {
        let registry = default_registry();
        let result = registry
            .dispatch("calculate", &serde_json::json!({"expression": "7 * 6"}))
            .expect("calculate should be registered");
        assert_eq!(result.output, "42");
    }
}
