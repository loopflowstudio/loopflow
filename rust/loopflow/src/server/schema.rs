//! The JSON Schema of the protocol, generated from the wire types. `lf server
//! describe` prints it; the fixtures in `rust/loopflow/tests/fixtures/dto/server/` validate
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
    let mut schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Loopflow server protocol",
        "x-revision": REVISION,
        "x-min-revision": MIN_REVISION,
        "oneOf": [client, server],
        "$defs": generator.take_definitions(true),
    });
    add_examples(&mut schema);
    schema
}

/// Put each recorded exchange beside its operation: the request's operation
/// in `examples`, what it answered in `x-result-example`.
fn add_examples(schema: &mut Value) {
    let parse =
        |text: &str| -> Value { serde_json::from_str(text).expect("embedded fixtures are JSON") };
    let mut requests: std::collections::BTreeMap<String, Vec<Value>> = Default::default();
    for text in REQUESTS {
        let operation = parse(text)["operation"].clone();
        let kind = operation["kind"]
            .as_str()
            .expect("an operation names its kind");
        requests.entry(kind.to_owned()).or_default().push(operation);
    }
    let mut results: std::collections::BTreeMap<&str, Value> = Default::default();
    for (_, kind, text) in RESPONSES {
        results
            .entry(kind)
            .or_insert_with(|| parse(text)["result"].clone());
    }
    for group in ["Read", "Write", "ServerOperation"] {
        let Some(arms) = schema["$defs"][group]["oneOf"].as_array_mut() else {
            continue;
        };
        for arm in arms {
            let Some(kind) = arm["properties"]["kind"]["const"]
                .as_str()
                .map(str::to_owned)
            else {
                continue;
            };
            if let Some(examples) = requests.remove(&kind) {
                arm["examples"] = Value::Array(examples);
            }
            if let Some(result) = results.get(kind.as_str()) {
                arm["x-result-example"] = result.clone();
            }
        }
    }
}

/// Recorded requests, embedded so `describe` needs no source tree. Each
/// names its operation; a test fails when a request fixture is missing here.
const REQUESTS: &[&str] = &[
    include_str!("../../tests/fixtures/dto/server/account_connect.json"),
    include_str!("../../tests/fixtures/dto/server/account_disconnect.json"),
    include_str!("../../tests/fixtures/dto/server/account_list.json"),
    include_str!("../../tests/fixtures/dto/server/account_redeem_reset.json"),
    include_str!("../../tests/fixtures/dto/server/account_next_request.json"),
    include_str!("../../tests/fixtures/dto/server/account_set.json"),
    include_str!("../../tests/fixtures/dto/server/account_use.json"),
    include_str!("../../tests/fixtures/dto/server/account_pause.json"),
    include_str!("../../tests/fixtures/dto/server/account_unpause.json"),
    include_str!("../../tests/fixtures/dto/server/attach.json"),
    include_str!("../../tests/fixtures/dto/server/session_start.json"),
    include_str!("../../tests/fixtures/dto/server/checkout_commit.json"),
    include_str!("../../tests/fixtures/dto/server/checkout_create.json"),
    include_str!("../../tests/fixtures/dto/server/checkout_sync.json"),
    include_str!("../../tests/fixtures/dto/server/configuration.json"),
    include_str!("../../tests/fixtures/dto/server/graph_run.json"),
    include_str!("../../tests/fixtures/dto/server/pr_abandon.json"),
    include_str!("../../tests/fixtures/dto/server/pr_arm.json"),
    include_str!("../../tests/fixtures/dto/server/pr_checks.json"),
    include_str!("../../tests/fixtures/dto/server/pr_land.json"),
    include_str!("../../tests/fixtures/dto/server/pr_open.json"),
    include_str!("../../tests/fixtures/dto/server/pr_publish.json"),
    include_str!("../../tests/fixtures/dto/server/pr_reconcile.json"),
    include_str!("../../tests/fixtures/dto/server/pr_request_review.json"),
    include_str!("../../tests/fixtures/dto/server/pr_reviews.json"),
    include_str!("../../tests/fixtures/dto/server/pr_submit.json"),
    include_str!("../../tests/fixtures/dto/server/repo_ci.json"),
    include_str!("../../tests/fixtures/dto/server/repo_release.json"),
    include_str!("../../tests/fixtures/dto/server/session_steer.json"),
    include_str!("../../tests/fixtures/dto/server/session_boundary.json"),
    include_str!("../../tests/fixtures/dto/server/session_status.json"),
    include_str!("../../tests/fixtures/dto/server/skill_list.json"),
    include_str!("../../tests/fixtures/dto/server/skill_run.json"),
    include_str!("../../tests/fixtures/dto/server/task_from_session.json"),
    include_str!("../../tests/fixtures/dto/server/task_hold.json"),
    include_str!("../../tests/fixtures/dto/server/task_interrupt.json"),
    include_str!("../../tests/fixtures/dto/server/task_report.json"),
    include_str!("../../tests/fixtures/dto/server/task_run.json"),
    include_str!("../../tests/fixtures/dto/server/task_ship.json"),
    include_str!("../../tests/fixtures/dto/server/task_steer.json"),
    include_str!("../../tests/fixtures/dto/server/view_plan.json"),
    include_str!("../../tests/fixtures/dto/server/wave_plan_bind.json"),
    include_str!("../../tests/fixtures/dto/server/wave_plan_connect.json"),
    include_str!("../../tests/fixtures/dto/server/wave_plan_reteam.json"),
    include_str!("../../tests/fixtures/dto/server/configure_remote.json"),
    include_str!("../../tests/fixtures/dto/server/mobile_status_request.json"),
    include_str!("../../tests/fixtures/dto/server/session_page.json"),
    include_str!("../../tests/fixtures/dto/server/session_continue.json"),
    include_str!("../../tests/fixtures/dto/server/task_create.json"),
    include_str!("../../tests/fixtures/dto/server/upload.json"),
];

/// Recorded answers and the operation each answers. A new `response`
/// fixture must be named here; subscriptions answer null.
const RESPONSES: &[(&str, &str, &str)] = &[
    (
        "account",
        "account.disconnect",
        include_str!("../../tests/fixtures/dto/server/account.json"),
    ),
    (
        "account_redeemed",
        "account.redeem_reset",
        include_str!("../../tests/fixtures/dto/server/account_redeemed.json"),
    ),
    (
        "account_report",
        "account.list",
        include_str!("../../tests/fixtures/dto/server/account_report.json"),
    ),
    (
        "account_next",
        "account.next",
        include_str!("../../tests/fixtures/dto/server/account_next.json"),
    ),
    (
        "checkout_committed",
        "checkout.commit",
        include_str!("../../tests/fixtures/dto/server/checkout_committed.json"),
    ),
    (
        "checkout_created",
        "checkout.create",
        include_str!("../../tests/fixtures/dto/server/checkout_created.json"),
    ),
    (
        "checkout_sync_planned",
        "checkout.sync",
        include_str!("../../tests/fixtures/dto/server/checkout_sync_planned.json"),
    ),
    (
        "checkout_syncing",
        "checkout.sync",
        include_str!("../../tests/fixtures/dto/server/checkout_syncing.json"),
    ),
    (
        "launched",
        "repo.release",
        include_str!("../../tests/fixtures/dto/server/launched.json"),
    ),
    (
        "attached",
        "session.attach",
        include_str!("../../tests/fixtures/dto/server/attached.json"),
    ),
    (
        "session_started",
        "session.start",
        include_str!("../../tests/fixtures/dto/server/session_started.json"),
    ),
    (
        "pr_abandoned",
        "pr.abandon",
        include_str!("../../tests/fixtures/dto/server/pr_abandoned.json"),
    ),
    (
        "pr_checks_report",
        "pr.checks",
        include_str!("../../tests/fixtures/dto/server/pr_checks_report.json"),
    ),
    (
        "pr_delivery_check",
        "pr.reconcile",
        include_str!("../../tests/fixtures/dto/server/pr_delivery_check.json"),
    ),
    (
        "pr_prepared",
        "pr.submit",
        include_str!("../../tests/fixtures/dto/server/pr_prepared.json"),
    ),
    (
        "pr_published",
        "pr.publish",
        include_str!("../../tests/fixtures/dto/server/pr_published.json"),
    ),
    (
        "pr_review_requested",
        "pr.request_review",
        include_str!("../../tests/fixtures/dto/server/pr_review_requested.json"),
    ),
    (
        "pr_reviews_report",
        "pr.reviews",
        include_str!("../../tests/fixtures/dto/server/pr_reviews_report.json"),
    ),
    (
        "repo_ci_report",
        "repo.ci",
        include_str!("../../tests/fixtures/dto/server/repo_ci_report.json"),
    ),
    (
        "response",
        "session.rename",
        include_str!("../../tests/fixtures/dto/server/response.json"),
    ),
    (
        "session_boundary_answer",
        "session.boundary",
        include_str!("../../tests/fixtures/dto/server/session_boundary_answer.json"),
    ),
    (
        "session_steered",
        "session.steer",
        include_str!("../../tests/fixtures/dto/server/session_steered.json"),
    ),
    (
        "skill_entries",
        "skill.list",
        include_str!("../../tests/fixtures/dto/server/skill_entries.json"),
    ),
    (
        "task_held",
        "task.hold",
        include_str!("../../tests/fixtures/dto/server/task_held.json"),
    ),
    (
        "task_running",
        "task.run",
        include_str!("../../tests/fixtures/dto/server/task_running.json"),
    ),
    (
        "task_shipping",
        "task.ship",
        include_str!("../../tests/fixtures/dto/server/task_shipping.json"),
    ),
    (
        "wave_plan_connected",
        "wave.plan.connect",
        include_str!("../../tests/fixtures/dto/server/wave_plan_connected.json"),
    ),
    (
        "wave_plan_reteamed",
        "wave.plan.reteam",
        include_str!("../../tests/fixtures/dto/server/wave_plan_reteamed.json"),
    ),
    (
        "ticket_reply",
        "request.upload",
        include_str!("../../tests/fixtures/dto/server/ticket_reply.json"),
    ),
    (
        "mobile_status",
        "mobile.status",
        include_str!("../../tests/fixtures/dto/server/mobile_status.json"),
    ),
    (
        "remote",
        "mobile.configure",
        include_str!("../../tests/fixtures/dto/server/remote.json"),
    ),
];

/// What bare `lf server describe` prints.
pub fn summary() -> String {
    let schema = describe();
    let count = |name: &str| {
        schema["$defs"][name]["oneOf"]
            .as_array()
            .expect("an operation enum is a oneOf of its kinds")
            .len()
    };
    let counted = |name: &str, one: &str, many: &str| match count(name) {
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    };
    format!(
        "Server protocol revision {REVISION} (oldest {MIN_REVISION}): {}, {}, {}. `--json` prints its JSON Schema.",
        counted("Read", "read", "reads"),
        counted("Write", "write", "writes"),
        counted("ServerOperation", "server operation", "server operations"),
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

    use super::{describe, TimeTuple, REQUESTS, RESPONSES};
    use crate::lf::commands::{read::Read, write::Write};

    const REGENERATE: &str =
        "cargo run -p loopflow --bin lf -- server describe --json > tests/fixtures/dto/server_schema.json";

    fn fixtures_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/dto")
    }

    fn fixtures() -> BTreeMap<String, Value> {
        std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dto/server"))
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
        let responses: BTreeMap<_, _> = RESPONSES
            .iter()
            .map(|(name, kind, _)| (*name, *kind))
            .collect();
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
    fn every_recorded_exchange_is_an_example() {
        for (name, fixture) in fixtures() {
            let embedded = match fixture["type"].as_str() {
                Some("request") => REQUESTS
                    .iter()
                    .any(|text| serde_json::from_str::<Value>(text).unwrap() == fixture),
                Some("response") => RESPONSES.iter().any(|(listed, _, _)| *listed == name),
                _ => true,
            };
            assert!(embedded, "{name} is not embedded in server::schema");
        }
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
            wire(serde_json::to_value(Read::MachineDoctor).unwrap()),
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
        assert!(names.is_superset(&BTreeSet::from(["task.create", "session.attach"])));
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
