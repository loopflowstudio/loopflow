//! The JSON Schema of the protocol, generated from the wire types. `lf server
//! describe` prints it; the fixtures in `tests/fixtures/dto/server/` validate
//! against it.
use std::borrow::Cow;

use schemars::generate::SchemaSettings;
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde_json::{json, Value};

use super::wire::{ClientMessage, Hello, ServerMessage, Welcome, MIN_REVISION, REVISION};

/// One document. Its root accepts any message either side sends; each
/// operation names what it answers in `x-result`. The serialize contract keeps
/// nullable fields required, as both sides send them.
pub fn describe() -> Value {
    let mut generator = SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator();
    let client = generator.subschema_for::<ClientMessage>();
    let server = generator.subschema_for::<ServerMessage>();
    generator.subschema_for::<Hello>();
    generator.subschema_for::<Welcome>();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Loopflow server protocol",
        "x-revision": REVISION,
        "x-min-revision": MIN_REVISION,
        "oneOf": [client, server],
        "$defs": generator.take_definitions(true),
    })
}

/// What bare `lf server describe` prints.
pub fn summary() -> String {
    let schema = describe();
    let count = |name: &str| {
        schema["$defs"][name]["oneOf"]
            .as_array()
            .expect("an operation enum is a oneOf of its kinds")
            .len()
    };
    format!(
        "Server protocol revision {REVISION} (oldest {MIN_REVISION}): {} reads, {} writes, {} server operations. `--json` prints its JSON Schema.",
        count("Read"),
        count("Write"),
        count("ServerOperation"),
    )
}

/// A bare `time::OffsetDateTime` as its serde writes it: year, ordinal day,
/// hour, minute, second, nanosecond and the offset's hours, minutes and
/// seconds. A field that wants RFC 3339 says `time::serde::rfc3339`.
pub(crate) type TimeTuple = (i32, u16, u8, u8, u8, u32, i8, i8, i8);

/// A result that is `serde_json::Value` on the wire: this build promises no
/// shape for it, and the schema says so rather than inventing one.
#[derive(Debug)]
pub struct Untyped;

impl JsonSchema for Untyped {
    fn schema_name() -> Cow<'static, str> {
        "Untyped".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Untyped on the wire: any JSON value, with no shape promised.",
        })
    }
}

/// A result whose shape the request's own fields choose.
pub(crate) fn either<A: JsonSchema, B: JsonSchema>(generator: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "anyOf": [generator.subschema_for::<A>(), generator.subschema_for::<B>()],
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use super::{describe, TimeTuple};
    use crate::lf::commands::{read::Read, write::Write};

    const REGENERATE: &str =
        "cargo run -p loopflow --bin lf -- server describe --json > tests/fixtures/dto/server_schema.json";

    /// The `response` fixtures and the operation each answers. A new one must
    /// name its operation here; `process.attach` and subscriptions answer null.
    const RESPONSES: &[(&str, &str)] = &[
        ("account", "account.disconnect"),
        ("account_redeemed", "account.redeem_reset"),
        ("account_report", "account.list"),
        ("account_routes", "account.route"),
        ("checkout_committed", "checkout.commit"),
        ("checkout_created", "checkout.create"),
        ("checkout_sync_planned", "checkout.sync"),
        ("checkout_syncing", "checkout.sync"),
        ("launched", "repo.release"),
        ("owner", "session.owner"),
        ("pr_abandoned", "pr.abandon"),
        ("pr_checks_report", "pr.checks"),
        ("pr_delivery_check", "pr.reconcile"),
        ("pr_prepared", "pr.submit"),
        ("pr_published", "pr.publish"),
        ("pr_review_requested", "pr.request_review"),
        ("pr_reviews_report", "pr.reviews"),
        ("repo_ci_report", "repo.ci"),
        ("response", "session.rename"),
        ("session_steered", "session.steer"),
        ("skill_entries", "skill.list"),
        ("task_held", "task.hold"),
        ("task_running", "task.run"),
        ("task_shipping", "task.ship"),
        ("wave_plan_connected", "wave.plan.connect"),
        ("wave_plan_reteamed", "wave.plan.reteam"),
    ];

    fn fixtures_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/dto")
    }

    fn fixtures() -> BTreeMap<String, Value> {
        std::fs::read_dir(fixtures_root().join("server"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
                let raw = std::fs::read_to_string(&path).unwrap();
                (name, serde_json::from_str(&raw).unwrap())
            })
            .collect()
    }

    /// The arms of a tagged definition, by the value of their `tag`.
    fn arms(schema: &Value, definition: &str, tag: &str) -> BTreeMap<String, Value> {
        schema["$defs"][definition]["oneOf"]
            .as_array()
            .unwrap_or_else(|| panic!("{definition} has no arms"))
            .iter()
            .map(|arm| {
                let name = arm["properties"][tag]["const"].as_str();
                let name = name.unwrap_or_else(|| panic!("{definition} arm has no {tag}: {arm}"));
                (name.to_owned(), arm.clone())
            })
            .collect()
    }

    fn operations(schema: &Value) -> BTreeMap<String, Value> {
        ["ServerOperation", "Read", "Write"]
            .into_iter()
            .flat_map(|definition| arms(schema, definition, "kind"))
            .collect()
    }

    /// `subschema` resolved against the document's definitions.
    fn validator(schema: &Value, subschema: &Value) -> jsonschema::Validator {
        let mut document = subschema.clone();
        document["$defs"] = schema["$defs"].clone();
        jsonschema::draft202012::new(&document).unwrap()
    }

    fn violations(validator: &jsonschema::Validator, value: &Value) -> Vec<String> {
        validator
            .iter_errors(value)
            .map(|error| format!("{}: {error}", error.instance_path()))
            .collect()
    }

    #[test]
    fn every_fixture_validates_against_its_definition() {
        let schema = describe();
        let messages = [
            arms(&schema, "ClientMessage", "type"),
            arms(&schema, "ServerMessage", "type"),
        ];
        let mut failed = BTreeMap::new();
        for (name, fixture) in fixtures() {
            let kind = fixture["type"].as_str().unwrap_or_default();
            // `status` names an arm on both sides: the probe and its answer.
            let reports: Vec<_> = messages
                .iter()
                .filter_map(|arms| arms.get(kind))
                .map(|arm| violations(&validator(&schema, arm), &fixture))
                .collect();
            assert!(
                !reports.is_empty(),
                "fixture {name} has no definition: type {kind:?}"
            );
            if reports.iter().all(|report| !report.is_empty()) {
                failed.insert(name, reports);
            }
        }
        assert!(failed.is_empty(), "{failed:#?}");
    }

    #[test]
    fn every_response_fixture_validates_against_its_operations_result() {
        let schema = describe();
        let operations = operations(&schema);
        let responses: BTreeMap<_, _> = RESPONSES.iter().copied().collect();
        let mut failed = BTreeMap::new();
        for (name, fixture) in fixtures() {
            if fixture["type"] != "response" {
                continue;
            }
            let kind = responses.get(name.as_str());
            let kind =
                kind.unwrap_or_else(|| panic!("response fixture {name} is not in RESPONSES"));
            let result = &operations[*kind]["x-result"];
            let report = violations(&validator(&schema, result), &fixture["result"]);
            if !report.is_empty() {
                failed.insert(format!("{name} as {kind}"), report);
            }
        }
        assert!(failed.is_empty(), "{failed:#?}");
    }

    #[test]
    fn every_operation_declares_what_it_answers() {
        let schema = describe();
        for (kind, operation) in operations(&schema) {
            assert!(
                operation.get("x-result").is_some(),
                "{kind} declares no result"
            );
        }
    }

    #[test]
    fn kinds_appear_exactly_as_on_the_wire() {
        let operations = operations(&describe());
        let wire = |value: Value| value["kind"].as_str().unwrap().to_owned();
        for kind in [
            wire(serde_json::to_value(Read::Doctor).unwrap()),
            wire(
                serde_json::to_value(Write::TaskComplete {
                    directory: "/".into(),
                    issue: "A".into(),
                })
                .unwrap(),
            ),
        ] {
            assert!(operations.contains_key(&kind), "{kind}");
        }
        let names: BTreeSet<_> = operations.keys().map(String::as_str).collect();
        assert!(names.is_superset(&BTreeSet::from(["task.create", "session.owner"])));
    }

    #[test]
    fn bare_time_serializes_as_its_described_tuple() {
        let wire = serde_json::to_value(time::OffsetDateTime::UNIX_EPOCH).unwrap();
        let tuple: TimeTuple = serde_json::from_value(wire).unwrap();
        assert_eq!(tuple, (1970, 1, 0, 0, 0, 0, 0, 0, 0));
    }

    #[test]
    fn the_checked_in_schema_is_what_describe_prints() {
        let path = fixtures_root().join("server_schema.json");
        let saved = std::fs::read_to_string(&path).unwrap_or_default();
        let current = serde_json::to_string_pretty(&describe()).unwrap() + "\n";
        assert!(
            saved == current,
            "{} is stale; regenerate it from the repository root:\n  {REGENERATE}",
            path.display()
        );
    }
}
