use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use std::borrow::Cow;

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
