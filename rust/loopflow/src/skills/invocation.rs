//! A selected skill and its exact arguments, separate from gathered user context.
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::flow::Skill;
use crate::skills::catalog::SkillDialect;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInvocation {
    pub skill: Skill,
    pub arguments: String,
}

impl SkillInvocation {
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    fn capture_directory(config: &crate::agent::AgentConfig) -> anyhow::Result<PathBuf> {
        let capture = config
            .env
            .get(crate::session_record::CAPTURE_KEY_ENV)
            .ok_or_else(|| anyhow::anyhow!("skill input requires its Session capture"))?;
        Ok(crate::session_record::capture_dir(capture)?)
    }

    fn declarations(&self) -> Option<serde_yaml_ng::Value> {
        self.skill
            .source
            .as_ref()?
            .frontmatter
            .as_deref()
            .and_then(|text| serde_yaml_ng::from_str(text).ok())
    }

    fn native_name(&self) -> String {
        self.declarations()
            .as_ref()
            .and_then(|fields| fields.get("name"))
            .and_then(serde_yaml_ng::Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                self.skill
                    .source
                    .as_ref()
                    .and_then(|source| source.path.parent())
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| self.skill.name.clone())
            })
    }

    fn unchanged_source(&self) -> bool {
        self.skill.source.as_ref().is_some_and(|source| {
            std::fs::read_to_string(&source.path).ok().as_deref() == Some(&self.skill.source_text())
        })
    }

    fn native_declarations(&self, harness: &str) -> bool {
        let fields = self.declarations();
        if self
            .skill
            .source
            .as_ref()
            .is_some_and(|source| source.frontmatter.is_some())
            && !fields
                .as_ref()
                .is_some_and(serde_yaml_ng::Value::is_mapping)
        {
            return false;
        }
        harness != "codex"
            || fields.as_ref().is_some_and(|fields| {
                ["name", "description"]
                    .iter()
                    .all(|key| fields.get(key).is_some_and(serde_yaml_ng::Value::is_string))
            })
    }

    pub(crate) fn claude_input(
        &self,
        config: &crate::agent::AgentConfig,
    ) -> anyhow::Result<(Vec<String>, String)> {
        if !self.native_for("claude") || !self.native_declarations("claude") {
            return Ok((Vec::new(), self.translated_input("claude")));
        }
        self.report_native_declarations("claude");
        self.prepare_claude(config, None)
    }

    fn prepare_claude(
        &self,
        config: &crate::agent::AgentConfig,
        context: Option<&str>,
    ) -> anyhow::Result<(Vec<String>, String)> {
        if context.is_none() && self.unchanged_source() {
            let path = &self
                .skill
                .source
                .as_ref()
                .expect("native skill has a source")
                .path;
            if path.file_name().is_some_and(|name| name == "SKILL.md") {
                let directory = path.parent().expect("skill has a directory");
                let namespace = directory
                    .file_name()
                    .expect("skill directory has a name")
                    .to_string_lossy();
                return Ok((
                    vec!["--plugin-dir".into(), directory.display().to_string()],
                    self.command(&format!("/{namespace}:{}", self.native_name())),
                ));
            }
        }
        // Captured definitions and legacy command files need an unambiguous native name.
        let root = tempfile::Builder::new()
            .prefix("skill-")
            .tempdir_in(Self::capture_directory(config)?)?
            .keep();
        self.materialize_claude(&root.join("skills/invoke"), context)?;
        let namespace = format!("lf-{}", uuid::Uuid::new_v4().simple());
        std::fs::create_dir(root.join(".claude-plugin"))?;
        std::fs::write(
            root.join(".claude-plugin/plugin.json"),
            serde_json::to_vec(&serde_json::json!({"name": namespace}))?,
        )?;
        Ok((
            vec!["--plugin-dir".into(), root.display().to_string()],
            self.command(&format!("/{namespace}:invoke")),
        ))
    }

    fn codex_skill(&self) -> Option<(&Path, String)> {
        let source = self.skill.source.as_ref()?;
        (self.native_for("codex")
            && source
                .path
                .file_name()
                .is_some_and(|name| name == "SKILL.md")
            && self.native_declarations("codex")
            && self.unchanged_source())
        .then(|| (source.path.as_path(), self.native_name()))
    }

    pub(crate) fn codex_prompt(&self) -> String {
        match self.codex_skill() {
            // Explicit Markdown references resolve outside Codex's catalog too;
            // a `skill` input item alone is silently ignored there.
            Some((path, name)) => {
                self.report_native_declarations("codex");
                self.command(&format!("[${name}]({})", path.display()))
            }
            None => self.translated_input("codex"),
        }
    }

    pub(crate) fn terminal_input(
        &self,
        harness: &str,
        config: &crate::agent::AgentConfig,
    ) -> anyhow::Result<(Vec<String>, String)> {
        let context = &config.task_prompt;
        if self.native_for("claude") && harness == "claude" && self.native_declarations("claude") {
            self.report_native_declarations("claude");
            return self.prepare_claude(config, (!context.is_empty()).then_some(context.as_str()));
        }
        let prompt = if harness == "codex" {
            self.codex_prompt()
        } else {
            self.translated_input(harness)
        };
        Ok((Vec::new(), format!("{prompt}\n\n{context}")))
    }

    fn report_native_declarations(&self, harness: &str) {
        let supported: &[&str] = match harness {
            "claude" => &[
                "name",
                "description",
                "argument-hint",
                "arguments",
                "allowed-tools",
                "model",
                "effort",
                "context",
                "agent",
                "hooks",
                "user-invocable",
                "disable-model-invocation",
                "license",
                "compatibility",
                "metadata",
            ],
            _ => &[
                "name",
                "description",
                "metadata",
                "license",
                "compatibility",
            ],
        };
        let fields = self.declarations();
        let Some(fields) = fields.as_ref().and_then(serde_yaml_ng::Value::as_mapping) else {
            return;
        };
        let unhandled: Vec<_> = fields
            .keys()
            .filter_map(serde_yaml_ng::Value::as_str)
            .filter(|name| !supported.contains(name))
            .collect();
        if !unhandled.is_empty() {
            eprintln!(
                "warning: {} on {harness}: declarations not enforced by this launch: {}",
                self.skill.name,
                unhandled.join(", ")
            );
        }
    }

    fn translated_input(&self, harness: &str) -> String {
        eprintln!("warning: {} on {harness}: running captured instructions; source model, permissions, hooks and subagent declarations are retained but not enforced by this launch", self.skill.name);
        self.ported_text(harness)
    }

    fn native_for(&self, harness: &str) -> bool {
        matches!(
            (
                self.skill.source.as_ref().map(|source| source.dialect),
                harness
            ),
            (Some(SkillDialect::Claude), "claude") | (Some(SkillDialect::Codex), "codex")
        )
    }

    fn ported_text(&self, harness: &str) -> String {
        let body = self.skill.content.as_deref().unwrap_or_default();
        let origin = self
            .skill
            .source
            .as_ref()
            .expect("ported skills have a source");
        let directory = origin.path.parent().unwrap_or(Path::new("."));
        let mut body = self.expand_arguments(body);
        if origin.dialect == SkillDialect::Claude {
            body = body.replace("${CLAUDE_SKILL_DIR}", &directory.display().to_string());
        }
        let tools = match (origin.dialect, harness) {
            (SkillDialect::Claude, "codex") => "Claude tool names map to Codex tools: Bash, Read, Grep and Glob use exec_command; Edit and Write use apply_patch; TodoWrite uses update_plan; Task uses spawn_agent when available. Use the available web tool for WebFetch and WebSearch.",
            (SkillDialect::Codex, "claude") => "Codex tool names map to Claude tools: exec_command uses Bash; apply_patch uses Edit or Write; update_plan uses TodoWrite when available; spawn_agent uses Task. Use WebFetch and WebSearch for web requests.",
            _ => "Use this harness's corresponding tools for the operations named below.",
        };
        format!("---\nname: {}\ndescription: {}\n---\nSource: {}\nBase directory of the original skill: {}\n\n{tools}\nExecute any shell-context directives below through ordinary tools before proceeding; they have not been pre-executed.\n\nOriginal declarations (model, permissions, subagents and preprocessing have no automatic cross-harness enforcement):\n```yaml\n{}\n```\n\n{body}",
            serde_json::to_string(&self.skill.name).expect("skill name is serializable"),
            serde_json::to_string(&format!("Ported invocation of {}", self.skill.name)).expect("skill name is serializable"),
            origin.path.display(), directory.display(), origin.frontmatter.as_deref().unwrap_or_default())
    }

    fn expand_arguments(&self, body: &str) -> String {
        static ARGUMENT: LazyLock<regex::Regex> = LazyLock::new(|| {
            regex::Regex::new(r"(\\*)\$(ARGUMENTS\[\d+\]|[A-Za-z_]\w*|\d+)")
                .expect("argument pattern is valid")
        });
        let fields = self.declarations();
        let names = match fields.as_ref().and_then(|fields| fields.get("arguments")) {
            Some(serde_yaml_ng::Value::String(value)) => value
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>(),
            Some(serde_yaml_ng::Value::Sequence(values)) => values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        let arguments = shlex::split(&self.arguments).unwrap_or_default();
        let codex_prompt = self.skill.source.as_ref().is_some_and(|source| {
            source.dialect == SkillDialect::Codex
                && source
                    .path
                    .file_name()
                    .is_some_and(|name| name != "SKILL.md")
        });
        let named_arguments: std::collections::BTreeMap<_, _> = if codex_prompt {
            arguments
                .iter()
                .filter_map(|argument| argument.split_once('='))
                .collect()
        } else {
            Default::default()
        };
        let mut substituted = false;
        let expanded = ARGUMENT
            .replace_all(body, |capture: &regex::Captures<'_>| {
                let token = &capture[2];
                let index = token
                    .strip_prefix("ARGUMENTS[")
                    .and_then(|value| value.strip_suffix(']'))
                    .unwrap_or(token)
                    .parse::<usize>()
                    .ok();
                let named = names.iter().position(|name| name == token);
                let supplied = named_arguments.get(token).copied();
                if token != "ARGUMENTS" && index.is_none() && named.is_none() && supplied.is_none()
                {
                    return capture[0].to_string();
                }
                if &capture[1] == "\\" {
                    return format!("${token}");
                }
                let value = if token == "ARGUMENTS" {
                    Some(self.arguments.as_str())
                } else if let Some(value) = supplied {
                    Some(value)
                } else if let Some(index) = named {
                    Some(arguments.get(index).map(String::as_str).unwrap_or_default())
                } else {
                    index.and_then(|index| {
                        let index = if codex_prompt {
                            index.checked_sub(1)?
                        } else {
                            index
                        };
                        arguments.get(index).map(String::as_str)
                    })
                };
                match value {
                    Some(value) => {
                        substituted = true;
                        format!("{}{value}", &capture[1])
                    }
                    None => capture[0].to_string(),
                }
            })
            .into_owned();
        if !substituted && !self.arguments.is_empty() {
            format!("{expanded}\n\nARGUMENTS: {}", self.arguments)
        } else {
            expanded
        }
    }

    fn command(&self, name: &str) -> String {
        if self.arguments.is_empty() {
            name.to_string()
        } else {
            format!("{name} {}", self.arguments)
        }
    }

    /// Keep the Flow's captured text and resolve resources at their original directory.
    fn materialize_claude(&self, directory: &Path, context: Option<&str>) -> anyhow::Result<()> {
        std::fs::create_dir_all(directory)?;
        let target = directory.join("SKILL.md");
        let original = self
            .skill
            .source
            .as_ref()
            .and_then(|source| source.path.parent())
            .unwrap_or(Path::new("."));
        let mut text = self
            .skill
            .source_text()
            .replace("${CLAUDE_SKILL_DIR}", &original.display().to_string());
        text.push_str(&format!("\n\nResolve supporting files and parent-relative paths from the original skill directory: {}\n", original.display()));
        if let Some(context) = context {
            // Native expansion applies to the whole skill body. Encode gathered
            // text so literal argument and shell syntax remains user data.
            let context = serde_json::to_string(context)?
                .replace('$', "\\u0024")
                .replace('!', "\\u0021")
                .replace('\u{fffe}', "\\ufffe")
                .replace('\u{ffff}', "\\uffff");
            text.push_str(&format!(
                "\n\nAdditional user context (decode this JSON string):\n{context}\n"
            ));
        }
        std::fs::write(&target, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SkillInvocation;
    use crate::flow::Skill;
    use crate::skills::catalog::SkillCatalog;
    use crate::skills::catalog::{SkillDialect, SkillOrigin};

    #[test]
    fn port_arguments_preserve_quoting_escapes_and_literal_inserted_tokens() {
        let invocation = SkillInvocation {
            skill: Skill {
                source: Some(SkillOrigin {
                    path: "/skills/audit/SKILL.md".into(),
                    dialect: SkillDialect::Claude,
                    frontmatter: Some("arguments: [first, second, absent]".into()),
                }),
                ..Skill::named("audit")
            },
            arguments: "\"hello world\" '$ARGUMENTS'".into(),
        };
        assert_eq!(
            invocation.expand_arguments(
                r"$0 | $ARGUMENTS[1] | $2 | $first | $absent | \$1 | \\$1 | $HOME"
            ),
            r"hello world | $ARGUMENTS | $2 | hello world |  | $1 | \\$ARGUMENTS | $HOME"
        );
        assert_eq!(
            invocation.expand_arguments("$ARGUMENTS"),
            invocation.arguments
        );
        assert_eq!(
            invocation.expand_arguments("No placeholders."),
            format!("No placeholders.\n\nARGUMENTS: {}", invocation.arguments)
        );
    }

    #[test]
    fn codex_prompts_expand_one_based_positions_and_named_assignments() {
        let invocation = SkillInvocation {
            skill: Skill {
                source: Some(SkillOrigin {
                    path: "/skills/prompts/audit.md".into(),
                    dialect: SkillDialect::Codex,
                    frontmatter: None,
                }),
                ..Skill::named("audit")
            },
            arguments: "\"hello world\" TARGET=\"the branch\"".into(),
        };
        assert_eq!(
            invocation.expand_arguments("$1 | $TARGET | $ARGUMENTS"),
            "hello world | the branch | \"hello world\" TARGET=\"the branch\""
        );
    }

    #[test]
    fn port_keeps_declarations_and_original_asset_directory_visible() {
        let invocation = SkillInvocation {
            skill: Skill {
                source: Some(SkillOrigin {
                    path: "/skills/audit/SKILL.md".into(),
                    dialect: SkillDialect::Claude,
                    frontmatter: Some("allowed-tools: Read\nmodel: sonnet".into()),
                }),
                content: Some("Read ${CLAUDE_SKILL_DIR}/reference.md for $ARGUMENTS".into()),
                ..Skill::named("audit")
            },
            arguments: "the branch".into(),
        };
        let text = invocation.ported_text("codex");
        assert!(text.contains("/skills/audit/reference.md for the branch"));
        assert!(text.contains("allowed-tools: Read\nmodel: sonnet"));
        assert!(text.contains("no automatic cross-harness enforcement"));
        assert!(text.contains("exec_command"));
    }

    #[test]
    fn captured_source_survives_replacement_and_keeps_bundle_files() {
        let repo = tempfile::tempdir().unwrap();
        let bundle = repo.path().join(".claude/skills/audit");
        std::fs::create_dir_all(&bundle).unwrap();
        let original = "---\nname: audit\n---\nCheck $ARGUMENTS.\n";
        std::fs::write(bundle.join("SKILL.md"), original).unwrap();
        std::fs::write(bundle.join("reference.txt"), "the reference").unwrap();
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let invocation = SkillInvocation {
            skill: catalog.resolve("audit").unwrap().unwrap().load().unwrap(),
            arguments: "  exact \"arguments\"\nsecond line  ".into(),
        };
        assert_eq!(invocation.skill.source_text(), original);
        assert_eq!(
            invocation.command("/audit"),
            "/audit   exact \"arguments\"\nsecond line  "
        );
        std::fs::write(bundle.join("SKILL.md"), "replacement").unwrap();
        assert_ne!(
            std::fs::read_to_string(bundle.join("SKILL.md")).unwrap(),
            invocation.skill.source_text()
        );
        let snapshot = repo.path().join("snapshot");
        invocation.materialize_claude(&snapshot, None).unwrap();
        let text = std::fs::read_to_string(snapshot.join("SKILL.md")).unwrap();
        assert!(text.starts_with(original));
        assert!(text.contains(&bundle.display().to_string()));
        assert_eq!(
            std::fs::read_to_string(bundle.join("reference.txt")).unwrap(),
            "the reference"
        );
    }
}
