//! The executable catalogue of LF preferences. Native provider settings and live
//! Work records belong to their domain owners, not this namespace.
use crate::prompt::context_budget::BudgetKey;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use super::settings::SettingScope;
use super::{
    placement, AutomationConfig, ConfigDeclarations, ConfigSource, LogConfig, MergeMethod,
    PlanningConfig, ReleaseConfig, SessionConfig, SummaryConfig, UserConfig,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    Launch,
    General,
    Repository,
    Personal,
    PersonalScoped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Combination {
    Replace,
    AppendReset,
    Merge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Portability {
    Portable,
    HostLocal,
    ReferencedAssets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    NextRead,
    NextLaunch,
    DomainAction,
    Inactive,
}

/// Display order and feature ownership, independent of settings precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Category {
    #[serde(rename = "Agents and context")]
    Agents,
    Machines,
    Accounts,
    Desktop,
    Planning,
    Release,
    Loopflow,
}

impl Category {
    pub fn title(self) -> &'static str {
        match self {
            Self::Agents => "Agents and context",
            Self::Machines => "Machines",
            Self::Accounts => "Accounts",
            Self::Desktop => "Desktop",
            Self::Planning => "Planning",
            Self::Release => "Release",
            Self::Loopflow => "Loopflow",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Definition {
    pub key: &'static str,
    pub category: Category,
    #[serde(serialize_with = "serialize_schema")]
    pub schema: fn() -> schemars::Schema,
    pub default: &'static str,
    pub applicability: Applicability,
    pub combination: Combination,
    pub portability: Portability,
    pub lifecycle: Lifecycle,
    pub description: &'static str,
}

impl Definition {
    pub fn applies(&self, layer: ConfigSource, scope: SettingScope, targeted: bool) -> bool {
        (!targeted || self.applicability == Applicability::Launch)
            && match self.applicability {
                Applicability::Personal => {
                    scope == SettingScope::Global && !matches!(layer, ConfigSource::Repository)
                }
                Applicability::PersonalScoped => !matches!(layer, ConfigSource::Repository),
                Applicability::Repository => scope != SettingScope::Wave,
                _ => true,
            }
    }

    pub fn accepts(&self, path: &[&str]) -> bool {
        let schema = (self.schema)().to_value();
        schema_path(&schema, &schema, &path[self.key.split('.').count()..])
    }
}

fn serialize_schema<S: serde::Serializer>(
    schema: &fn() -> schemars::Schema,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    schema().serialize(serializer)
}

fn schema_path(root: &serde_json::Value, node: &serde_json::Value, path: &[&str]) -> bool {
    if path.is_empty() {
        return true;
    }
    if let Some(reference) = node.get("$ref").and_then(serde_json::Value::as_str) {
        return reference
            .strip_prefix('#')
            .and_then(|reference| root.pointer(reference))
            .is_some_and(|node| schema_path(root, node, path));
    }
    if let Some(child) = node
        .get("properties")
        .and_then(|properties| properties.get(path[0]))
    {
        return schema_path(root, child, &path[1..]);
    }
    if let Some(child) = node
        .get("additionalProperties")
        .filter(|value| value.is_object())
    {
        return schema_path(root, child, &path[1..]);
    }
    ["anyOf", "oneOf", "allOf"].iter().any(|key| {
        node.get(key)
            .and_then(serde_json::Value::as_array)
            .is_some_and(|variants| {
                variants
                    .iter()
                    .any(|variant| schema_path(root, variant, path))
            })
    })
}

macro_rules! definitions {
    ($(#[$struct_attribute:meta])* $name:ident, $table:ident, $prefix:literal, {$( $(#[$extra_attribute:meta])* $extra:ident: $extra_type:ty = $extra_default:expr;)*}; $( $(#[$attribute:meta])* $key:ident: $ty:ty = $default:literal, $scope:ident, $combine:ident, $portable:ident, $lifecycle:ident, $category:ident, $description:literal;)*) => {
        /// Typed effective preferences. Authored declarations retain omission separately.
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        $(#[$struct_attribute])*
        pub struct $name {
            $( $(#[$attribute])* pub $key: $ty,)*
            $( $(#[$extra_attribute])* pub $extra: $extra_type,)*
        }
        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($key: serde_yaml_ng::from_str::<$ty>($default).expect("typed catalogue default"),)*
                    $($extra: $extra_default,)*
                }
            }
        }
        pub static $table: &[Definition] = &[$(Definition {
            key: concat!($prefix, stringify!($key)), schema: || schemars::schema_for!($ty), default: $default,
            category: Category::$category,
            applicability: Applicability::$scope, combination: Combination::$combine,
            portability: Portability::$portable, lifecycle: Lifecycle::$lifecycle, description: $description,
        }),*];
    };
}

definitions! {
    #[derive(schemars::JsonSchema)]
    ContextConfig, CONTEXT_DEFINITIONS, "context.", {};
    docs: Vec<String> = "[]", Launch, AppendReset, ReferencedAssets, NextLaunch, Agents, "Additional documentation; [] clears earlier contributions";
    exclude: Vec<String> = "[]", Launch, AppendReset, Portable, NextLaunch, Agents, "Exclude matching context paths";
    diff: bool = "false", Launch, Replace, Portable, NextLaunch, Agents, "Include the branch diff";
    diff_files: bool = "false", Launch, Replace, Portable, NextLaunch, Agents, "Include full requested file contents";
    paste: bool = "false", Launch, Replace, HostLocal, NextLaunch, Agents, "Include this host's clipboard";
    budgets: BTreeMap<BudgetKey, usize> = "{}", Launch, Merge, Portable, NextRead, Agents, "Positive context size targets";
}

definitions! {
    Config, DEFINITIONS, "", {
        skills: BTreeMap<String, Value> = BTreeMap::new();
        graphs: BTreeMap<String, Value> = BTreeMap::new();
        #[serde(skip)] setting_layers: Option<Vec<ConfigDeclarations>> = None;
        #[serde(skip)] context_budget_sources: BTreeMap<BudgetKey, String> = BTreeMap::new();
    };
    agent: Option<String> = "null", Launch, Replace, Portable, NextLaunch, Agents, "Harness[:model] choice; every choice precedes every fallback";
    default_agent: Option<String> = "null", Launch, Replace, Portable, NextLaunch, Agents, "Harness[:model] when no applicable agent choice exists";
    context: ContextConfig = "{}", Launch, Merge, Portable, NextLaunch, Agents, "Files and other inputs supplied to the agent";
    yolo: bool = "false", Launch, Replace, Portable, NextLaunch, Agents, "Skip agent permission prompts: Claude, Codex and OpenCode; Pi keeps its own tool policy";
    chrome: Option<bool> = "null", Launch, Replace, HostLocal, NextLaunch, Agents, "Claude Chrome integration; unset leaves Claude's own setting; Codex, OpenCode and Pi have no equivalent";
    isolate: bool = "false", Launch, Replace, Portable, NextLaunch, Agents, "Run in the selected account home";
    summaries: Vec<SummaryConfig> = "[]", Launch, AppendReset, ReferencedAssets, Inactive, Agents, "Parsed summary requests; no runtime consumer";
    summary_tokens: usize = "5000", Launch, Replace, Portable, Inactive, Agents, "Parsed summary budget; no runtime consumer";
    #[serde(alias = "workflow")]
    graph: Option<String> = "null", General, Replace, Portable, NextLaunch, Agents, "Graph new Tasks take up";
    supported_harnesses: Vec<String> = "[]", General, AppendReset, Portable, NextRead, Agents, "Harnesses offered by model selection";
    automation: AutomationConfig = "{retries: 1}", General, Merge, Portable, NextRead, Agents, "Automatic retry count";
    pools: BTreeMap<String, placement::PoolDeclaration> = "{}", Personal, Merge, HostLocal, NextLaunch, Machines, "Local pools accepting new work; held work stays";
    ondemands: BTreeMap<String, placement::OnDemandDeclaration> = "{}", Personal, Replace, HostLocal, NextLaunch, Machines, "Local rental templates and admission limits; active rentals keep their admitted budget";
    machine: placement::Placement = "{}", PersonalScoped, Merge, Portable, NextLaunch, Machines, "Preferred durable Machine IDs or local capacity; existing Task assignments survive";
    machines: BTreeMap<String, placement::MachineDeclaration> = "{}", Personal, Merge, Portable, NextRead, Machines, "Aliases for durable Machine identities";
    connections: HashMap<crate::durable::MachineId, String> = "{}", Personal, Merge, HostLocal, NextRead, Machines, "This host's connection address for each durable Machine ID";
    auth_browsers: BTreeMap<String, placement::AuthBrowsers> = "{}", Personal, Merge, HostLocal, DomainAction, Accounts, "Ordered local browser profiles for authentication; discovered browser identities stay local";
    account: BTreeMap<String, placement::AccountOrder> = "{}", PersonalScoped, Merge, Portable, NextLaunch, Accounts, "Allowed account references in order; each provider list replaces atomically";
    accounts: BTreeMap<String, BTreeMap<String, placement::AccountDeclaration>> = "{}", Personal, Merge, Portable, NextRead, Accounts, "Declared account identity, routing eligibility and subscription dates; credentials and observations stay local";
    repo_root: Option<PathBuf> = "null", Personal, Replace, HostLocal, NextRead, Desktop, "Local repository discovery directory";
    theme: Option<String> = "null", Personal, Replace, ReferencedAssets, NextRead, Desktop, "Appearance theme or light:name,dark:name pair";
    task_layout: Option<crate::work::selection::TaskLayoutTemplate> = "null", General, Replace, Portable, NextLaunch, Desktop, "Tools and splits for Task workspaces without a saved Desktop layout; the winning scope replaces the whole layout";
    portfolio: Option<crate::portfolio::Selection> = "null", Personal, Replace, HostLocal, NextRead, Desktop, "Repository navigation selection";
    planning: Option<PlanningConfig> = "null", Repository, Replace, Portable, DomainAction, Planning, "Tagged planning transport configuration";
    release: ReleaseConfig = "{targets: {}}", Repository, Merge, ReferencedAssets, DomainAction, Release, "Repository release targets";
    land: String = "gh", Repository, Replace, Portable, DomainAction, Release, "Landing strategy: gh or local";
    merge_method: Option<MergeMethod> = "null", Repository, Replace, Portable, DomainAction, Release, "Preferred allowed merge method";
    pr: bool = "false", Repository, Replace, Portable, DomainAction, Release, "Create a pull request after pushing";
    user: UserConfig = "{}", Personal, Merge, Portable, NextRead, Loopflow, "Display name; never an ownership credential";
    session: SessionConfig = "{}", Personal, Merge, HostLocal, NextRead, Loopflow, "Terminal and detached Session lifetime";
    stream: Option<String> = "null", Personal, Replace, Portable, DomainAction, Loopflow, "Installation release stream";
    logs: LogConfig = "{linear_requests: false}", Personal, Merge, HostLocal, NextRead, Loopflow, "Local diagnostic logging";
}

pub fn definition(key: &str) -> Option<&'static Definition> {
    DEFINITIONS
        .iter()
        .chain(CONTEXT_DEFINITIONS)
        .filter(|definition| {
            key == definition.key
                || key
                    .strip_prefix(definition.key)
                    .is_some_and(|rest| rest.starts_with('.'))
        })
        .max_by_key(|definition| definition.key.len())
}

pub fn defaults() -> Value {
    let mut values = Value::Mapping(Default::default());
    for definition in DEFINITIONS.iter().chain(CONTEXT_DEFINITIONS) {
        super::settings::insert(
            &mut values,
            &definition.key.split('.').collect::<Vec<_>>(),
            serde_yaml_ng::from_str(definition.default).expect("catalogue default is valid YAML"),
        );
    }
    values
}

/// Full paths and their own policies, grouped for CLI and Desktop browsing.
pub fn definitions() -> Vec<&'static Definition> {
    let mut definitions: Vec<_> = DEFINITIONS.iter().chain(CONTEXT_DEFINITIONS).collect();
    definitions.sort_by_key(|definition| definition.category);
    definitions
}
