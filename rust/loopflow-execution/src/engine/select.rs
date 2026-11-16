//! Field selection on a read's answer: `select` on a subscription or request,
//! `--select` on the command. Each entry is a dotted path into the JSON body;
//! the projection keeps the path's ancestors as objects and applies a path
//! through an array to each element. A path the body lacks is simply absent.

use serde_json::{Map, Value};

/// `value` reduced to `paths`. An empty list is an empty object; a path's
/// prefix (`a` beside `a.b`) keeps `a` whole.
pub fn select(value: &Value, paths: &[String]) -> Value {
    let paths: Vec<Vec<&str>> = paths
        .iter()
        .map(|path| path.split('.').filter(|part| !part.is_empty()).collect())
        .filter(|parts: &Vec<&str>| !parts.is_empty())
        .collect();
    project(value, &paths)
}

fn project(value: &Value, paths: &[Vec<&str>]) -> Value {
    if paths.iter().any(|path| path.is_empty()) {
        return value.clone();
    }
    match value {
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| project(item, paths)).collect())
        }
        Value::Object(fields) => {
            let mut kept = Map::new();
            for (key, inner) in fields {
                let below: Vec<Vec<&str>> = paths
                    .iter()
                    .filter(|path| path[0] == key)
                    .map(|path| path[1..].to_vec())
                    .collect();
                if !below.is_empty() {
                    kept.insert(key.clone(), project(inner, &below));
                }
            }
            Value::Object(kept)
        }
        // A scalar has no fields below it; the path named something absent.
        _ => Value::Object(Map::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::select;
    use serde_json::json;

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|path| path.to_string()).collect()
    }

    #[test]
    fn nested_paths_keep_their_ancestors() {
        let body = json!({"task": {"identifier": "A-1", "title": "t", "notes": "n"}, "pr": {"url": "u", "state": "open"}});
        assert_eq!(
            select(&body, &paths(&["task.identifier", "pr.url"])),
            json!({"task": {"identifier": "A-1"}, "pr": {"url": "u"}})
        );
    }

    #[test]
    fn a_path_through_an_array_applies_to_each_element() {
        let body = json!({"items": [{"id": 1, "x": "a"}, {"id": 2, "x": "b"}], "truncated": false});
        assert_eq!(
            select(&body, &paths(&["items.id"])),
            json!({"items": [{"id": 1}, {"id": 2}]})
        );
    }

    #[test]
    fn a_missing_path_is_absent_not_an_error() {
        let body = json!({"a": {"b": 1}});
        assert_eq!(select(&body, &paths(&["a.z", "q"])), json!({"a": {}}));
        assert_eq!(select(&json!(7), &paths(&["a"])), json!({}));
    }

    #[test]
    fn a_prefix_keeps_the_whole_subtree() {
        let body = json!({"a": {"b": 1, "c": 2}, "d": 3});
        assert_eq!(
            select(&body, &paths(&["a", "a.b"])),
            json!({"a": {"b": 1, "c": 2}})
        );
    }

    #[test]
    fn an_empty_list_is_an_empty_object() {
        assert_eq!(select(&json!({"a": 1}), &[]), json!({}));
    }
}
