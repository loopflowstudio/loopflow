use async_trait::async_trait;
use reqwest::{Method, StatusCode, Url};
use serde_json::{json, Map, Value};
use tokio::time::sleep;
use tracing::warn;

use crate::engine::config::NotionConfig;
use crate::lfd::pm::notion_blocks::{blocks_to_markdown, markdown_to_blocks};
use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProject, PmProvider, PmResult, PriorityBucket,
    RATE_LIMIT_RETRIES,
};

const NOTION_BASE_URL: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";
const DEFAULT_TEAM_NAME: &str = "Waves";
const NOTION_PAGE_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct NotionClient {
    client: reqwest::Client,
    token: String,
    config: NotionConfig,
    base_url: String,
}

impl NotionClient {
    pub fn new(token: String, config: NotionConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            config,
            base_url: NOTION_BASE_URL.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_url(token: String, config: NotionConfig, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            config,
            base_url,
        }
    }

    fn title_property_name(&self) -> &str {
        self.config.title_property.as_deref().unwrap_or("Name")
    }

    fn status_property_name(&self) -> &str {
        self.config.status_property.as_deref().unwrap_or("Status")
    }

    fn done_value(&self) -> &str {
        self.config.done_value.as_deref().unwrap_or("Done")
    }

    fn priority_property_name(&self) -> &str {
        self.config
            .priority_property
            .as_deref()
            .unwrap_or("Priority")
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let url = Url::parse(&format!("{}{}", self.base_url, path))
            .expect("notion base URL should be valid");
        self.client
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Notion-Version", NOTION_VERSION)
    }

    fn request_with_start_cursor(
        &self,
        method: Method,
        path: &str,
        start_cursor: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let mut url = Url::parse(&format!("{}{}", self.base_url, path))
            .expect("notion base URL should be valid");
        if let Some(cursor) = start_cursor {
            url.query_pairs_mut().append_pair("start_cursor", cursor);
        }
        self.client
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Notion-Version", NOTION_VERSION)
    }

    async fn send_json<F>(&self, mut make_request: F) -> PmResult<Value>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        for attempt in 0..=RATE_LIMIT_RETRIES {
            let response = make_request()
                .send()
                .await
                .map_err(|err| PmError::Message(format!("notion request failed: {err}")))?;

            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < RATE_LIMIT_RETRIES {
                let delay = super::retry_after_delay(response.headers());
                warn!(
                    attempt = attempt + 1,
                    delay_seconds = delay.as_secs(),
                    "notion rate limited; retrying"
                );
                sleep(delay).await;
                continue;
            }

            return parse_response(response).await;
        }

        Err(PmError::Message(
            "notion request failed after retries".to_string(),
        ))
    }

    async fn collect_paginated_results<F>(&self, mut make_request: F) -> PmResult<Vec<Value>>
    where
        F: FnMut(Option<&str>) -> reqwest::RequestBuilder,
    {
        let mut results = Vec::new();
        let mut next_cursor: Option<String> = None;

        loop {
            let cursor = next_cursor.clone();
            let response = self.send_json(|| make_request(cursor.as_deref())).await?;

            results.extend(
                response
                    .get("results")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            );

            if !response
                .get("has_more")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                break;
            }

            next_cursor = response
                .get("next_cursor")
                .and_then(Value::as_str)
                .map(str::to_string);
        }

        Ok(results)
    }

    async fn resolve_team_for_project_bootstrap(&self) -> PmResult<String> {
        if let Some(parent) = &self.config.parent_page {
            return Ok(parent.clone());
        }
        if let Some(team_id) = self.find_team(DEFAULT_TEAM_NAME).await? {
            return Ok(team_id);
        }
        self.create_team(DEFAULT_TEAM_NAME).await
    }

    async fn search(&self, query: &str, value: &str) -> PmResult<Vec<Value>> {
        self.collect_paginated_results(|start_cursor| {
            let mut body = json!({
                "query": query,
                "page_size": NOTION_PAGE_SIZE,
                "filter": {
                    "property": "object",
                    "value": value,
                }
            });
            if let Some(cursor) = start_cursor {
                body["start_cursor"] = Value::String(cursor.to_string());
            }
            self.request(Method::POST, "/search").json(&body)
        })
        .await
    }

    async fn fetch_page(&self, page_id: &str) -> PmResult<Value> {
        self.send_json(|| self.request(Method::GET, &format!("/pages/{page_id}")))
            .await
    }

    async fn fetch_page_body_markdown(&self, page_id: &str) -> PmResult<String> {
        let blocks = self.fetch_block_children(page_id).await?;
        Ok(blocks_to_markdown(&blocks))
    }

    async fn fetch_direct_block_children(&self, block_id: &str) -> PmResult<Vec<Value>> {
        self.collect_paginated_results(|start_cursor| {
            self.request_with_start_cursor(
                Method::GET,
                &format!("/blocks/{block_id}/children"),
                start_cursor,
            )
        })
        .await
    }

    async fn fetch_block_children(&self, block_id: &str) -> PmResult<Vec<Value>> {
        let mut children = self.fetch_direct_block_children(block_id).await?;

        for block in &mut children {
            if block
                .get("has_children")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let id = value_string(block, "id")?;
                let nested = Box::pin(self.fetch_block_children(&id)).await?;
                if let Some(block_type) = block
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                {
                    if let Some(data) = block.get_mut(&block_type).and_then(Value::as_object_mut) {
                        data.insert("children".to_string(), Value::Array(nested));
                    }
                }
            }
        }

        Ok(children)
    }

    async fn fetch_top_level_block_ids(&self, block_id: &str) -> PmResult<Vec<String>> {
        self.fetch_direct_block_children(block_id)
            .await?
            .iter()
            .map(|block| value_string(block, "id"))
            .collect()
    }

    async fn delete_page_body(&self, page_id: &str) -> PmResult<()> {
        for block_id in self.fetch_top_level_block_ids(page_id).await? {
            let _ = self
                .send_json(|| self.request(Method::DELETE, &format!("/blocks/{block_id}")))
                .await?;
        }
        Ok(())
    }

    async fn append_page_body(&self, page_id: &str, markdown: &str) -> PmResult<()> {
        let blocks = markdown_to_blocks(markdown);
        if blocks.is_empty() {
            return Ok(());
        }

        let _ = self
            .send_json(|| {
                self.request(Method::PATCH, &format!("/blocks/{page_id}/children"))
                    .json(&json!({ "children": blocks }))
            })
            .await?;
        Ok(())
    }

    fn page_title_property(&self, text: &str) -> Value {
        json!({
            self.title_property_name(): {
                "title": [text_span(text)]
            }
        })
    }

    fn page_status_property(&self, done: bool) -> Value {
        let name = if done { self.done_value() } else { "Todo" };
        json!({
            self.status_property_name(): {
                "status": { "name": name }
            }
        })
    }

    fn item_properties(&self, item: &PmItemCreate) -> Map<String, Value> {
        let mut properties = Map::new();
        properties.extend(value_object(self.page_title_property(&item.name)));
        properties.extend(value_object(self.page_status_property(false)));
        properties
    }

    async fn query_database_pages(&self, database_id: &str) -> PmResult<Vec<Value>> {
        self.collect_paginated_results(|start_cursor| {
            let mut body = json!({ "page_size": NOTION_PAGE_SIZE });
            if let Some(cursor) = start_cursor {
                body["start_cursor"] = Value::String(cursor.to_string());
            }
            self.request(Method::POST, &format!("/databases/{database_id}/query"))
                .json(&body)
        })
        .await
    }

    pub async fn create_team(&self, name: &str) -> PmResult<String> {
        let response = self
            .send_json(|| {
                self.request(Method::POST, "/pages").json(&json!({
                    "parent": { "workspace": true },
                    "properties": {
                        "title": [text_span(name)]
                    }
                }))
            })
            .await?;
        value_string(&response, "id")
    }

    pub async fn find_team(&self, name: &str) -> PmResult<Option<String>> {
        let target = name.trim();
        for page in self.search(target, "page").await? {
            let title = page_title(&page).unwrap_or_default();
            if title.eq_ignore_ascii_case(target) {
                return Ok(Some(value_string(&page, "id")?));
            }
        }
        Ok(None)
    }

    pub async fn create_project_in_team(
        &self,
        team_id: &str,
        name: &str,
        description: &str,
    ) -> PmResult<String> {
        let response = self
            .send_json(|| {
                self.request(Method::POST, "/databases").json(&json!({
                    "parent": { "type": "page_id", "page_id": team_id },
                    "title": [text_span(name)],
                    "description": [text_span(&truncate_text(description, 2000))],
                    "properties": {
                        self.title_property_name(): { "title": {} },
                        self.status_property_name(): {
                            "status": {
                                "options": [
                                    { "name": "Todo", "color": "default" },
                                    { "name": self.done_value(), "color": "green" }
                                ]
                            }
                        },
                        self.priority_property_name(): {
                            "select": {
                                "options": [
                                    { "name": PriorityBucket::Urgent.semantic_label(), "color": "red" },
                                    { "name": PriorityBucket::High.semantic_label(), "color": "orange" },
                                    { "name": PriorityBucket::Medium.semantic_label(), "color": "yellow" },
                                    { "name": PriorityBucket::Low.semantic_label(), "color": "gray" }
                                ]
                            }
                        }
                    }
                }))
            })
            .await?;
        value_string(&response, "id")
    }
}

#[async_trait]
impl PmProvider for NotionClient {
    async fn create_project(&self, name: &str, description: &str) -> PmResult<String> {
        let team_id = self.resolve_team_for_project_bootstrap().await?;
        self.create_project_in_team(&team_id, name, description)
            .await
    }

    async fn list_projects(&self, team_id: &str) -> PmResult<Vec<PmProject>> {
        let mut projects = Vec::new();
        for database in self.search("", "database").await? {
            if parent_page_id(&database).as_deref() != Some(team_id) {
                continue;
            }
            projects.push(PmProject {
                id: value_string(&database, "id")?,
                name: database_title(&database).unwrap_or_default(),
            });
        }
        projects.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(projects)
    }

    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        let pages = self.query_database_pages(project_id).await?;
        let mut items = Vec::new();
        for page in pages {
            let id = value_string(&page, "id")?;
            let description = self.fetch_page_body_markdown(&id).await?;
            items.push(PmItem {
                id,
                name: page_item_name(&page, self.title_property_name())?,
                description,
                rank: 0,
                completed: page_item_completed(
                    &page,
                    self.status_property_name(),
                    self.done_value(),
                ),
                assignee: None,
            });
        }
        Ok(items)
    }

    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String> {
        let properties = self.item_properties(item);
        let response = self
            .send_json(|| {
                self.request(Method::POST, "/pages").json(&json!({
                    "parent": { "database_id": project_id },
                    "properties": properties,
                }))
            })
            .await?;
        let page_id = value_string(&response, "id")?;
        self.append_page_body(&page_id, &item.description).await?;
        Ok(page_id)
    }

    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()> {
        let mut properties = Map::new();
        if let Some(name) = &update.name {
            properties.extend(value_object(self.page_title_property(name)));
        }

        if !properties.is_empty() {
            let _ = self
                .send_json(|| {
                    self.request(Method::PATCH, &format!("/pages/{item_id}"))
                        .json(&json!({ "properties": properties }))
                })
                .await?;
        }

        if let Some(description) = &update.description {
            self.delete_page_body(item_id).await?;
            self.append_page_body(item_id, description).await?;
        }

        Ok(())
    }

    async fn complete_item(&self, item_id: &str) -> PmResult<()> {
        let page = self.fetch_page(item_id).await?;
        let property = item_property(&page, self.status_property_name());
        let value = match property
            .and_then(|value| value.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("status")
        {
            "checkbox" => json!({ "checkbox": true }),
            "select" => json!({ "select": { "name": self.done_value() } }),
            _ => json!({ "status": { "name": self.done_value() } }),
        };

        let _ = self
            .send_json(|| {
                self.request(Method::PATCH, &format!("/pages/{item_id}"))
                    .json(&json!({
                        "properties": {
                            self.status_property_name(): value,
                        }
                    }))
            })
            .await?;
        Ok(())
    }

    async fn comment(&self, item_id: &str, body: &str) -> PmResult<()> {
        let _ = self
            .send_json(|| {
                self.request(Method::POST, "/comments").json(&json!({
                    "parent": { "page_id": item_id },
                    "rich_text": parse_comment_rich_text(body),
                }))
            })
            .await?;
        Ok(())
    }

    async fn claim_item(&self, _item_id: &str, _branch: &str) -> PmResult<()> {
        Ok(())
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let mut end = max_len.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end - 1])
}

fn text_span(text: &str) -> Value {
    json!({
        "type": "text",
        "text": { "content": text }
    })
}

fn parse_comment_rich_text(body: &str) -> Vec<Value> {
    if body.trim().is_empty() {
        vec![text_span("")]
    } else {
        vec![text_span(body)]
    }
}

fn value_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn value_string(value: &Value, key: &str) -> PmResult<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| PmError::Message(format!("notion response missing string field `{key}`")))
}

fn page_title(page: &Value) -> Option<String> {
    page.get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get("title"))
        .and_then(|property| property.get("title"))
        .and_then(Value::as_array)
        .map(|value| rich_text_array_plain_text(value))
        .filter(|title| !title.is_empty())
}

fn database_title(database: &Value) -> Option<String> {
    database
        .get("title")
        .and_then(Value::as_array)
        .map(|value| rich_text_array_plain_text(value))
        .filter(|title| !title.is_empty())
}

fn parent_page_id(value: &Value) -> Option<String> {
    value
        .get("parent")
        .and_then(|parent| parent.get("page_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn item_property<'a>(page: &'a Value, name: &str) -> Option<&'a Value> {
    page.get("properties")?.get(name)
}

fn page_item_name(page: &Value, title_property: &str) -> PmResult<String> {
    let property = item_property(page, title_property).ok_or_else(|| {
        PmError::Message(format!(
            "notion page is missing title property `{title_property}`"
        ))
    })?;
    let title = property
        .get("title")
        .and_then(Value::as_array)
        .map(|value| rich_text_array_plain_text(value))
        .unwrap_or_default();
    if title.is_empty() {
        return Err(PmError::Message(format!(
            "notion page title property `{title_property}` is empty"
        )));
    }
    Ok(title)
}

fn page_item_completed(page: &Value, status_property: &str, done_value: &str) -> bool {
    let Some(property) = item_property(page, status_property) else {
        return false;
    };

    if let Some(checked) = property.get("checkbox").and_then(Value::as_bool) {
        return checked;
    }

    property_select_name(property).is_some_and(|name| name.eq_ignore_ascii_case(done_value))
}

fn property_select_name(property: &Value) -> Option<String> {
    let property_type = property
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match property_type {
        "status" => property
            .get("status")
            .and_then(|status| status.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string),
        "select" => property
            .get("select")
            .and_then(|select| select.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn rich_text_array_plain_text(values: &[Value]) -> String {
    values
        .iter()
        .filter_map(|value| {
            value.get("plain_text").and_then(Value::as_str).or_else(|| {
                value
                    .get("text")
                    .and_then(|text| text.get("content"))
                    .and_then(Value::as_str)
            })
        })
        .collect::<Vec<_>>()
        .join("")
}

async fn parse_response(response: reqwest::Response) -> PmResult<Value> {
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|err| PmError::Message(format!("failed to read notion response: {err}")))?;

    if !status.is_success() {
        return Err(PmError::Message(parse_error_message(status, &body)));
    }

    serde_json::from_slice(&body)
        .map_err(|err| PmError::Message(format!("failed to decode notion response: {err}")))
}

fn parse_error_message(status: StatusCode, body: &[u8]) -> String {
    if let Ok(payload) = serde_json::from_slice::<Value>(body) {
        if let Some(message) = payload.get("message").and_then(Value::as_str) {
            return format!("notion request failed with status {status}: {message}");
        }
    }

    let body_text = String::from_utf8_lossy(body).trim().to_string();
    if body_text.is_empty() {
        format!("notion request failed with status {status}")
    } else {
        format!("notion request failed with status {status}: {body_text}")
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::*;
    use crate::engine::config::NotionConfig;
    use crate::lfd::pm::test_server::{self, json_response, response};
    use crate::lfd::pm::PmProvider;

    #[tokio::test]
    async fn create_team_creates_workspace_page() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::CREATED,
            json!({ "id": "team-1" }),
        )])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        let team_id = client.create_team("Waves").await.expect("team id");
        assert_eq!(team_id, "team-1");

        let request = requests.lock().await[0].clone();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/pages");
        assert_eq!(
            request.authorization.as_deref(),
            Some("Bearer secret-token")
        );
        assert!(request.body.contains("\"workspace\":true"));
        assert!(request.body.contains("Waves"));
    }

    #[tokio::test]
    async fn find_team_filters_search_results_by_title() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({
                "results": [
                    {
                        "id": "team-1",
                        "properties": { "title": { "title": [{ "plain_text": "Waves" }] } }
                    },
                    {
                        "id": "team-2",
                        "properties": { "title": { "title": [{ "plain_text": "Other" }] } }
                    }
                ]
            }),
        )])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        let team_id = client.find_team("waves").await.expect("find team");
        assert_eq!(team_id.as_deref(), Some("team-1"));
    }

    #[tokio::test]
    async fn create_project_in_team_creates_database_schema() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::CREATED,
            json!({ "id": "db-1" }),
        )])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        let project_id = client
            .create_project_in_team("team-1", "PM", "Project notes")
            .await
            .expect("project id");
        assert_eq!(project_id, "db-1");

        let request = requests.lock().await[0].clone();
        assert_eq!(request.path, "/databases");
        assert!(request.body.contains("\"page_id\":\"team-1\""));
        assert!(request.body.contains("\"Name\""));
        assert!(request.body.contains("\"Status\""));
        assert!(request.body.contains("\"Priority\""));
    }

    #[tokio::test]
    async fn list_projects_filters_to_team_parent() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({
                "results": [
                    {
                        "id": "db-2",
                        "parent": { "page_id": "team-2" },
                        "title": [{ "plain_text": "Other" }]
                    },
                    {
                        "id": "db-1",
                        "parent": { "page_id": "team-1" },
                        "title": [{ "plain_text": "PM" }]
                    }
                ]
            }),
        )])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        let projects = client.list_projects("team-1").await.expect("projects");
        assert_eq!(
            projects,
            vec![PmProject {
                id: "db-1".to_string(),
                name: "PM".to_string()
            }]
        );
    }

    #[tokio::test]
    async fn list_items_queries_pages_and_fetches_block_bodies() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "results": [
                        {
                            "id": "page-1",
                            "properties": {
                                "Name": { "type": "title", "title": [{ "plain_text": "Build it" }] },
                                "Priority": { "type": "select", "select": { "name": "High" } },
                                "Status": { "type": "status", "status": { "name": "Done" } }
                            }
                        }
                    ],
                    "has_more": false
                }),
            ),
            json_response(
                StatusCode::OK,
                json!({
                    "results": [
                        {
                            "id": "block-1",
                            "type": "paragraph",
                            "paragraph": {
                                "rich_text": [{ "plain_text": "First paragraph" }]
                            },
                            "has_children": false
                        }
                    ],
                    "has_more": false
                }),
            ),
        ])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        let items = client.list_items("db-1").await.expect("items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "page-1");
        assert_eq!(items[0].name, "Build it");
        assert!(items[0].completed);
        assert_eq!(items[0].description, "First paragraph");

        let requests = requests.lock().await.clone();
        assert_eq!(requests[0].path, "/databases/db-1/query");
        assert_eq!(requests[1].path, "/blocks/page-1/children");
    }

    #[tokio::test]
    async fn create_item_creates_page_then_appends_blocks() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(StatusCode::CREATED, json!({ "id": "page-1" })),
            json_response(StatusCode::OK, json!({ "results": [] })),
        ])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        let item_id = client
            .create_item(
                "db-1",
                &PmItemCreate {
                    name: "Build it".to_string(),
                    description: "# Heading\n\nBody".to_string(),
                    rank: 0,
                },
            )
            .await
            .expect("item id");
        assert_eq!(item_id, "page-1");

        let requests = requests.lock().await.clone();
        assert_eq!(requests[0].path, "/pages");
        assert!(requests[0].body.contains("\"database_id\":\"db-1\""));
        assert_eq!(requests[1].path, "/blocks/page-1/children");
        assert!(requests[1].body.contains("heading_1"));
        assert!(requests[1].body.contains("paragraph"));
    }

    #[tokio::test]
    async fn update_item_updates_properties_and_rewrites_description() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(StatusCode::OK, json!({ "id": "page-1" })),
            json_response(
                StatusCode::OK,
                json!({
                    "results": [{ "id": "block-1" }],
                    "has_more": false
                }),
            ),
            json_response(StatusCode::OK, json!({ "archived": true })),
            json_response(StatusCode::OK, json!({ "results": [] })),
        ])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        client
            .update_item(
                "page-1",
                &PmItemUpdate {
                    name: Some("New title".to_string()),
                    description: Some("Updated body".to_string()),
                    rank: None,
                },
            )
            .await
            .expect("update item");

        let requests = requests.lock().await.clone();
        assert_eq!(requests[0].path, "/pages/page-1");
        assert!(requests[0].body.contains("New title"));
        assert_eq!(requests[1].path, "/blocks/page-1/children");
        assert_eq!(requests[2].method, "DELETE");
        assert_eq!(requests[2].path, "/blocks/block-1");
        assert_eq!(requests[3].path, "/blocks/page-1/children");
        assert!(requests[3].body.contains("Updated body"));
    }

    #[tokio::test]
    async fn complete_item_uses_checkbox_when_status_property_is_checkbox() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({
                    "id": "page-1",
                    "properties": {
                        "Status": { "type": "checkbox", "checkbox": false }
                    }
                }),
            ),
            json_response(StatusCode::OK, json!({ "id": "page-1" })),
        ])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        client.complete_item("page-1").await.expect("complete item");

        let requests = requests.lock().await.clone();
        assert_eq!(requests[0].path, "/pages/page-1");
        assert_eq!(requests[1].path, "/pages/page-1");
        assert!(requests[1].body.contains("\"checkbox\":true"));
    }

    #[tokio::test]
    async fn comment_posts_rich_text_comment() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::CREATED,
            json!({ "id": "comment-1" }),
        )])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        client
            .comment("page-1", "Looks good")
            .await
            .expect("comment");

        let request = requests.lock().await[0].clone();
        assert_eq!(request.path, "/comments");
        assert!(request.body.contains("\"page_id\":\"page-1\""));
        assert!(request.body.contains("Looks good"));
    }

    #[tokio::test]
    async fn retries_after_rate_limit() {
        let (base_url, _requests) = test_server::spawn(vec![
            response(
                StatusCode::TOO_MANY_REQUESTS,
                vec![("content-type", "application/json"), ("retry-after", "0")],
                json!({ "message": "slow down" }).to_string(),
            ),
            json_response(StatusCode::CREATED, json!({ "id": "team-1" })),
        ])
        .await;
        let client = NotionClient::with_base_url(
            "secret-token".to_string(),
            NotionConfig::default(),
            base_url,
        );

        let team_id = client.create_team("Waves").await.expect("team id");
        assert_eq!(team_id, "team-1");
    }

    #[test]
    fn parse_error_message_prefers_json_message() {
        let message = parse_error_message(
            StatusCode::BAD_REQUEST,
            &json!({ "message": "bad schema" }).to_string().into_bytes(),
        );
        assert_eq!(
            message,
            "notion request failed with status 400 Bad Request: bad schema"
        );
    }
}
