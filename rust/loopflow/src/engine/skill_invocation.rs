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

    pub(crate) fn claude_plugin(
        &self,
        config: &crate::engine::agent::AgentConfig,
    ) -> anyhow::Result<(PathBuf, String)> {
        let capture = config
            .env
            .get(crate::session_record::CAPTURE_KEY_ENV)
            .ok_or_else(|| anyhow::anyhow!("native skill input requires its Session capture"))?;
        let directory = crate::session_record::capture_dir(capture)?;
        let root = tempfile::Builder::new()
            .prefix("skill-")
            .tempdir_in(directory)?
            .keep();
        if !self.native_for("claude") {
            eprintln!("warning: porting {} to claude; native model, tool permissions, subagents and shell preprocessing remain instructions, not enforced controls", self.skill.name);
        }
        self.materialize(&root.join("skills/invoke"), "claude")?;
        let namespace = format!("lf-{}", uuid::Uuid::new_v4().simple());
        std::fs::create_dir(root.join(".claude-plugin"))?;
        std::fs::write(
            root.join(".claude-plugin/plugin.json"),
            serde_json::to_vec(&serde_json::json!({
                "name": namespace,
            }))?,
        )?;
        Ok((root, self.command(&format!("/{namespace}:invoke"))))
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
            body.to_string()
        };
        let tools = match harness {
            "codex" => "Claude tool names map to Codex tools: Bash, Read, Grep and Glob use exec_command; Edit and Write use apply_patch; TodoWrite uses update_plan; Task uses spawn_agent when available. Use the available web tool for WebFetch and WebSearch.",
            "claude" => "Codex tool names map to Claude tools: exec_command uses Bash; apply_patch uses Edit or Write; update_plan uses TodoWrite when available; spawn_agent uses Task. Use WebFetch and WebSearch for web requests.",
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

    /// Materialize the captured definition without modifying a third-party file.
    /// Sibling resources continue to resolve against the original bundle.
    fn materialize(&self, directory: &Path, harness: &str) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(directory)?;
        let target = directory.join("SKILL.md");
        std::fs::write(&target, self.instruction_text(harness))?;
        if let Some(parent) = self
            .skill
            .source
            .as_ref()
            .and_then(|source| source.path.parent())
        {
            if parent.is_dir() {
                for entry in std::fs::read_dir(parent)? {
                    let entry = entry?;
                    if entry.file_name() == "SKILL.md" {
                        continue;
                    }
                    let link = directory.join(entry.file_name());
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(entry.path(), link)?;
                    #[cfg(not(unix))]
                    if entry.file_type()?.is_file() {
                        std::fs::copy(entry.path(), link)?;
                    }
                }
            }
        }
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
        assert_eq!(std::fs::read_to_string(file).unwrap(), text);
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
        assert_eq!(std::fs::read_to_string(file).unwrap(), original);
        assert_eq!(
            std::fs::read_to_string(snapshot.join("reference.txt")).unwrap(),
            "the reference"
        );
    }
}
