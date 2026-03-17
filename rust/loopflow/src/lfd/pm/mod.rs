pub mod asana;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PmProviderKind {
    Asana,
    Linear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmConfig {
    pub provider: PmProviderKind,
    pub project: String,
    #[serde(default)]
    pub team: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rank: u32,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmItemCreate {
    pub name: String,
    pub description: String,
    pub rank: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PmItemUpdate {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub rank: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PmError {
    #[error("{0}")]
    Message(String),
}

pub type PmResult<T> = Result<T, PmError>;

#[async_trait]
pub trait PmProvider: Send + Sync {
    async fn create_project(&self, name: &str, description: &str) -> PmResult<String>;
    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>>;
    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String>;
    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()>;
    async fn complete_item(&self, item_id: &str) -> PmResult<()>;
    async fn comment(&self, item_id: &str, body: &str) -> PmResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RoadmapItemFrontmatter {
    #[serde(default)]
    pub pm_id: Option<String>,
}

impl RoadmapItemFrontmatter {
    fn is_empty(&self) -> bool {
        self.pm_id.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadmapItemDocument {
    pub frontmatter: RoadmapItemFrontmatter,
    pub body: String,
}

impl RoadmapItemDocument {
    pub fn parse(content: &str) -> PmResult<Self> {
        let Some((frontmatter, body)) = split_frontmatter(content) else {
            return Ok(Self {
                frontmatter: RoadmapItemFrontmatter::default(),
                body: content.to_string(),
            });
        };

        let frontmatter = serde_yaml_ng::from_str::<RoadmapItemFrontmatter>(&frontmatter)
            .map_err(|err| PmError::Message(format!("invalid roadmap frontmatter: {err}")))?;

        Ok(Self { frontmatter, body })
    }

    pub fn render(&self) -> PmResult<String> {
        if self.frontmatter.is_empty() {
            return Ok(self.body.clone());
        }

        let frontmatter = serde_yaml_ng::to_string(&self.frontmatter).map_err(|err| {
            PmError::Message(format!("failed to encode roadmap frontmatter: {err}"))
        })?;

        Ok(format!("---\n{}---\n{}", frontmatter, self.body))
    }
}

fn split_frontmatter(content: &str) -> Option<(String, String)> {
    let rest = content.strip_prefix("---\n")?;
    let (frontmatter, body) = rest.split_once("\n---\n")?;
    Some((frontmatter.to_string(), body.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roadmap_item_document_parses_pm_id_frontmatter() {
        let doc = RoadmapItemDocument::parse(
            "---\npm_id: \"9876543210\"\n---\n# 01: Ship offline sync\n",
        )
        .expect("frontmatter should parse");

        assert_eq!(doc.frontmatter.pm_id.as_deref(), Some("9876543210"));
        assert_eq!(doc.body, "# 01: Ship offline sync\n");
    }

    #[test]
    fn roadmap_item_document_parses_without_frontmatter() {
        let doc = RoadmapItemDocument::parse("# 01: Ship offline sync\n")
            .expect("body-only documents should parse");

        assert_eq!(doc.frontmatter.pm_id, None);
        assert_eq!(doc.body, "# 01: Ship offline sync\n");
    }

    #[test]
    fn roadmap_item_document_render_round_trips_pm_id() {
        let original = RoadmapItemDocument {
            frontmatter: RoadmapItemFrontmatter {
                pm_id: Some("9876543210".to_string()),
            },
            body: "# 01: Ship offline sync\n".to_string(),
        };

        let rendered = original.render().expect("document should render");
        let reparsed =
            RoadmapItemDocument::parse(&rendered).expect("rendered document should parse");

        assert_eq!(reparsed, original);
    }

    #[test]
    fn roadmap_item_document_render_omits_empty_frontmatter() {
        let doc = RoadmapItemDocument {
            frontmatter: RoadmapItemFrontmatter::default(),
            body: "# 01: Ship offline sync\n".to_string(),
        };

        let rendered = doc.render().expect("document should render");
        assert_eq!(rendered, "# 01: Ship offline sync\n");
    }
}
