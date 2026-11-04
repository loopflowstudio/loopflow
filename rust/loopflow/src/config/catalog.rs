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

#[derive(Debug, Serialize)]
pub struct Definition {
    pub key: &'static str,
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
        schema_path(&schema, &schema, &path[1..])
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
    ($( $(#[$attribute:meta])* $key:ident: $ty:ty = $default:literal, $scope:ident, $combine:ident, $portable:ident, $lifecycle:ident, $description:literal;)*) => {
        /// Typed effective preferences. Authored declarations retain omission separately.
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct Config {
            $( $(#[$attribute])* pub $key: $ty,)*
            pub skills: BTreeMap<String, Value>,
            pub graphs: BTreeMap<String, Value>,
            #[serde(skip)]
            pub setting_layers: Option<Vec<ConfigDeclarations>>,
            #[serde(skip)]
            pub context_budget_sources: BTreeMap<BudgetKey, String>,
        }
        impl Default for Config {
            fn default() -> Self {
                Self {
                    $($key: serde_yaml_ng::from_str::<$ty>($default).expect("typed catalogue default"),)*
                    skills: BTreeMap::new(), graphs: BTreeMap::new(),
                    setting_layers: None, context_budget_sources: BTreeMap::new(),
                }
            }
        }
        pub static DEFINITIONS: &[Definition] = &[$(Definition {
            key: stringify!($key), schema: || schemars::schema_for!($ty), default: $default,
            applicability: Applicability::$scope, combination: Combination::$combine,
            portability: Portability::$portable, lifecycle: Lifecycle::$lifecycle, description: $description,
        }),*];
    };
}

definitions! {
    auth_browsers: BTreeMap<String, placement::AuthBrowsers> = "{}", Personal, Merge, HostLocal, DomainAction, "Ordered local browser profiles for authentication; discovered browser identities stay local";
    pools: BTreeMap<String, placement::PoolDeclaration> = "{}", Personal, Merge, HostLocal, NextLaunch, "Local pools accepting new work; held work stays";
    ondemands: BTreeMap<String, placement::OnDemandDeclaration> = "{}", Personal, Replace, HostLocal, NextLaunch, "Local rental templates and admission limits; active rentals keep their admitted budget";
    machine: placement::Placement = "{}", PersonalScoped, Merge, Portable, NextLaunch, "Preferred durable Machine IDs or local capacity; existing Task assignments survive";
    account: BTreeMap<String, placement::AccountOrder> = "{}", PersonalScoped, Merge, Portable, NextLaunch, "Allowed account references in order; each provider list replaces atomically";
    machines: BTreeMap<String, placement::MachineDeclaration> = "{}", Personal, Merge, Portable, NextRead, "Aliases for durable Machine identities";
    connections: HashMap<crate::durable::MachineId, String> = "{}", Personal, Merge, HostLocal, NextRead, "This host's connection address for each durable Machine ID";
    accounts: BTreeMap<String, BTreeMap<String, placement::AccountDeclaration>> = "{}", Personal, Merge, Portable, NextRead, "Declared account identity, routing eligibility and subscription dates; credentials and observations stay local";
    agent: Option<String> = "null", Launch, Replace, Portable, NextLaunch, "Harness[:model] choice; every choice precedes every fallback";
    default_agent: Option<String> = "null", Launch, Replace, Portable, NextLaunch, "Harness[:model] when no applicable agent choice exists";
    yolo: bool = "false", Launch, Replace, Portable, NextLaunch, "Skip agent permission prompts";
    chrome: bool = "false", Launch, Replace, HostLocal, NextLaunch, "Enable local Chrome integration";
    isolate: bool = "false", Launch, Replace, Portable, NextLaunch, "Run in the selected account home";
    docs: Vec<String> = "[]", Launch, AppendReset, ReferencedAssets, NextLaunch, "Additional documentation; [] clears earlier contributions";
    context: Vec<String> = "[]", Launch, AppendReset, ReferencedAssets, NextLaunch, "Additional context paths; [] clears earlier contributions";
    exclude: Vec<String> = "[]", Launch, AppendReset, Portable, NextLaunch, "Exclude matching context paths";
    diff: bool = "false", Launch, Replace, Portable, NextLaunch, "Include the branch diff";
    diff_files: bool = "false", Launch, Replace, Portable, NextLaunch, "Include full requested file contents";
    paste: bool = "false", Launch, Replace, HostLocal, NextLaunch, "Include this host's clipboard";
    summaries: Vec<SummaryConfig> = "[]", Launch, AppendReset, ReferencedAssets, Inactive, "Parsed summary requests; no runtime consumer";
    summary_tokens: usize = "5000", Launch, Replace, Portable, Inactive, "Parsed summary budget; no runtime consumer";
    #[serde(alias = "workflow")]
    graph: Option<String> = "null", General, Replace, Portable, NextLaunch, "Graph new Tasks take up";
    supported_harnesses: Vec<String> = "[]", General, AppendReset, Portable, NextRead, "Harnesses offered by model selection";
    context_budgets: BTreeMap<BudgetKey, usize> = "{}", General, Merge, Portable, NextRead, "Positive context size targets";
    automation: AutomationConfig = "{retries: 1}", General, Merge, Portable, NextRead, "Automatic retry count";
    planning: Option<PlanningConfig> = "null", Repository, Replace, Portable, DomainAction, "Tagged planning transport configuration";
    release: ReleaseConfig = "{targets: {}}", Repository, Merge, ReferencedAssets, DomainAction, "Repository release targets";
    land: String = "gh", Repository, Replace, Portable, DomainAction, "Landing strategy: gh or local";
    merge_method: Option<MergeMethod> = "null", Repository, Replace, Portable, DomainAction, "Preferred allowed merge method";
    pr: bool = "false", Repository, Replace, Portable, DomainAction, "Create a pull request after pushing";
    user: UserConfig = "{}", Personal, Merge, Portable, NextRead, "Display name; never an ownership credential";
    repo_root: Option<PathBuf> = "null", Personal, Replace, HostLocal, NextRead, "Local repository discovery directory";
    session: SessionConfig = "{}", Personal, Merge, HostLocal, NextRead, "Terminal and detached Session lifetime";
    theme: Option<String> = "null", Personal, Replace, ReferencedAssets, NextRead, "Appearance theme or light:name,dark:name pair";
    stream: Option<String> = "null", Personal, Replace, Portable, DomainAction, "Installation release stream";
    logs: LogConfig = "{linear_requests: false}", Personal, Merge, HostLocal, NextRead, "Local diagnostic logging";
    portfolio: Option<crate::portfolio::Selection> = "null", Personal, Replace, HostLocal, NextRead, "Repository navigation selection";
}

pub fn definition(key: &str) -> Option<&'static Definition> {
    DEFINITIONS
        .iter()
        .find(|definition| definition.key == key.split('.').next().unwrap_or(key))
}

pub fn defaults() -> Value {
    let mut values = serde_yaml_ng::Mapping::new();
    for definition in DEFINITIONS {
        values.insert(
            definition.key.into(),
            serde_yaml_ng::from_str(definition.default).expect("catalogue default is valid YAML"),
        );
    }
    values.into()
}
