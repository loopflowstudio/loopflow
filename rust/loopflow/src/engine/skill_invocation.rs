//! A selected skill and its exact arguments, separate from gathered user context.
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::engine::flow::Skill;
use crate::engine::skill_catalog::SkillDialect;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInvocation {
    pub skill: Skill,
    pub arguments: String,
}

impl SkillInvocation {
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    fn capture_directory(config: &crate::engine::agent::AgentConfig) -> anyhow::Result<PathBuf> {
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
            std::fs::read_to_string(&source.path).ok().as_deref() == Some(&self.source_text())
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
        config: &crate::engine::agent::AgentConfig,
    ) -> anyhow::Result<(Vec<String>, String)> {
        if !self.native_for("claude") || !self.native_declarations("claude") {
            return Ok((Vec::new(), self.translated_input("claude")));
        }
        if self.unchanged_source() {
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
        self.materialize(&root.join("skills/invoke"), "claude")?;
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
        (self.native_for("codex") && self.native_declarations("codex") && self.unchanged_source())
            .then(|| (source.path.as_path(), self.native_name()))
    }

    pub(crate) fn codex_input(&self) -> Vec<serde_json::Value> {
        match self.codex_skill() {
            Some((path, name)) => vec![
                serde_json::json!({"type": "text", "text": self.command(&format!("${name}"))}),
                serde_json::json!({"type": "skill", "name": name, "path": path}),
            ],
            None => {
                vec![serde_json::json!({"type": "text", "text": self.translated_input("codex")})]
            }
        }
    }

    pub(crate) fn terminal_input(
        &self,
        harness: &str,
        config: &crate::engine::agent::AgentConfig,
    ) -> String {
        let prompt = match (harness, self.codex_skill()) {
            ("codex", Some((path, name))) => {
                self.command(&format!("[${name}]({})", path.display()))
            }
            _ => self.translated_input(harness),
        };
        format!(
            "{prompt}\n\n{}\n\n{}",
            config.system_prompt, config.task_prompt
        )
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

    pub fn source_text(&self) -> String {
        let body = self.skill.content.as_deref().unwrap_or_default();
        match self
            .skill
            .source
            .as_ref()
            .and_then(|source| source.frontmatter.as_ref())
        {
            Some(frontmatter) => format!("---{frontmatter}---\n{body}"),
            None => body.to_string(),
        }
    }

    pub(crate) fn instruction_text(&self, harness: &str) -> String {
        if self.native_for(harness) {
            self.source_text()
        } else {
            self.ported_text(harness)
        }
    }

    fn ported_text(&self, harness: &str) -> String {
        let body = self.skill.content.as_deref().unwrap_or_default();
        let origin = self
            .skill
            .source
            .as_ref()
            .expect("ported skills have a source");
        let directory = origin.path.parent().unwrap_or(Path::new("."));
        let body = if origin.dialect == SkillDialect::Claude {
            self.expand_arguments(body)
                .replace("${CLAUDE_SKILL_DIR}", &directory.display().to_string())
        } else {
            self.expand_arguments(body)
        };
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
        let fields = self
            .skill
            .source
            .as_ref()
            .and_then(|origin| origin.frontmatter.as_deref())
            .and_then(|value| serde_yaml_ng::from_str::<serde_yaml_ng::Value>(value).ok());
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
                if token != "ARGUMENTS" && index.is_none() && named.is_none() {
                    return capture[0].to_string();
                }
                if &capture[1] == "\\" {
                    return format!("${token}");
                }
                let value = if token == "ARGUMENTS" {
                    Some(self.arguments.as_str())
                } else if let Some(index) = named {
                    Some(arguments.get(index).map(String::as_str).unwrap_or_default())
                } else {
                    index.and_then(|index| arguments.get(index).map(String::as_str))
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
    fn materialize(&self, directory: &Path, harness: &str) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(directory)?;
        let target = directory.join("SKILL.md");
        let original = self
            .skill
            .source
            .as_ref()
            .and_then(|source| source.path.parent())
            .unwrap_or(Path::new("."));
        let text = self
            .instruction_text(harness)
            .replace("${CLAUDE_SKILL_DIR}", &original.display().to_string());
        std::fs::write(&target, format!("{text}\n\nResolve supporting files and parent-relative paths from the original skill directory: {}\n", original.display()))?;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::SkillInvocation;
    use crate::engine::flow::Skill;
    use crate::engine::skill_catalog::SkillCatalog;
    use crate::engine::skill_catalog::{SkillDialect, SkillOrigin};

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
        let text = invocation.instruction_text("codex");
        assert!(text.contains("/skills/audit/reference.md for the branch"));
        assert!(text.contains("allowed-tools: Read\nmodel: sonnet"));
        assert!(text.contains("no automatic cross-harness enforcement"));
        assert!(text.contains("exec_command"));
        let snapshot = tempfile::tempdir().unwrap();
        let file = invocation.materialize(snapshot.path(), "codex").unwrap();
        assert!(std::fs::read_to_string(file).unwrap().contains(&text));
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
            skill: catalog.resolve("audit").unwrap().load().unwrap(),
            arguments: "  exact \"arguments\"\nsecond line  ".into(),
        };
        assert_eq!(invocation.source_text(), original);
        assert_eq!(
            invocation.command("/audit"),
            "/audit   exact \"arguments\"\nsecond line  "
        );
        std::fs::write(bundle.join("SKILL.md"), "replacement").unwrap();
        assert_ne!(
            std::fs::read_to_string(bundle.join("SKILL.md")).unwrap(),
            invocation.source_text()
        );
        let snapshot = repo.path().join("snapshot");
        let file = invocation.materialize(&snapshot, "claude").unwrap();
        let text = std::fs::read_to_string(file).unwrap();
        assert!(text.starts_with(original));
        assert!(text.contains(&bundle.display().to_string()));
        assert_eq!(
            std::fs::read_to_string(bundle.join("reference.txt")).unwrap(),
            "the reference"
        );
    }
}
