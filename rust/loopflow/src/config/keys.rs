//! Setting keys sit on their primitive: `repo.plan.team`,
//! `skill.<name>.agent`, `wave.graph`, `wave.<id>.account`. The typed catalogue
//! (`Config`) keeps the released layout (`planning`, `skills`, `graph`,
//! `waves`), which is also each key's shorthand; the released `linear.team`
//! spells `repo.plan.team`, and `plan.team` is its shorthand. Readers accept every spelling and fold a
//! document into the typed layout; writers gather the key's group into that
//! released layout, the one both builds read, before they edit it.
use serde_yaml_ng::{Mapping, Value};

/// A Wave's own file already stands on the primitive: `graph` stays bare there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    General,
    Wave,
}

/// The typed path any spelling of a key names.
pub fn typed(parts: &[&str]) -> Vec<String> {
    let own = |parts: &[&str]| {
        parts
            .iter()
            .map(|part| (*part).to_owned())
            .collect::<Vec<_>>()
    };
    match parts {
        ["repo", "plan", rest @ ..] | ["plan", rest @ ..] => {
            [vec!["planning".into()], own(rest)].concat()
        }
        ["linear", "team"] => vec!["planning".into(), "team".into()],
        ["skill", name, rest @ ..] => [vec!["skills".into(), (*name).into()], own(rest)].concat(),
        ["wave", "graph"] => vec!["graph".into()],
        ["wave" | "waves", id, rest @ ..] => {
            [vec!["waves".into(), (*id).into()], typed(rest)].concat()
        }
        _ => own(parts),
    }
}

pub fn typed_key(key: &str) -> String {
    typed(&key.split('.').collect::<Vec<_>>()).join(".")
}

/// The canonical spelling of a typed path.
pub fn canonical(parts: &[&str], root: Root) -> Vec<String> {
    let own = |parts: &[&str]| {
        parts
            .iter()
            .map(|part| (*part).to_owned())
            .collect::<Vec<_>>()
    };
    match parts {
        ["planning", rest @ ..] => [vec!["repo".into(), "plan".into()], own(rest)].concat(),
        ["skills", name, rest @ ..] => [vec!["skill".into(), (*name).into()], own(rest)].concat(),
        ["graph"] if root == Root::General => vec!["wave".into(), "graph".into()],
        ["waves", id, rest @ ..] => [
            vec!["wave".into(), (*id).into()],
            canonical(rest, Root::Wave),
        ]
        .concat(),
        _ => own(parts),
    }
}

pub fn canonical_key(key: &str) -> String {
    canonical(&key.split('.').collect::<Vec<_>>(), Root::General).join(".")
}

/// Every spelling of a typed path, canonical first, as a document may hold it.
pub fn spellings(parts: &[&str], root: Root) -> Vec<Vec<String>> {
    let own = |parts: &[&str]| {
        parts
            .iter()
            .map(|part| (*part).to_owned())
            .collect::<Vec<_>>()
    };
    let group = |heads: Vec<Vec<String>>, rest: &[&str]| -> Vec<Vec<String>> {
        heads
            .into_iter()
            .map(|head| [head, own(rest)].concat())
            .collect()
    };
    match parts {
        ["planning", "team"] => vec![
            vec!["repo".into(), "plan".into(), "team".into()],
            vec!["plan".into(), "team".into()],
            vec!["planning".into(), "team".into()],
            vec!["linear".into(), "team".into()],
        ],
        ["planning", rest @ ..] => group(
            vec![
                vec!["repo".into(), "plan".into()],
                vec!["plan".into()],
                vec!["planning".into()],
            ],
            rest,
        ),
        ["skills", name, rest @ ..] => group(
            vec![
                vec!["skill".into(), (*name).into()],
                vec!["skills".into(), (*name).into()],
            ],
            rest,
        ),
        ["graph"] if root == Root::General => {
            vec![vec!["wave".into(), "graph".into()], vec!["graph".into()]]
        }
        ["waves", id, rest @ ..] => {
            let tails = spellings(rest, Root::Wave);
            ["wave", "waves"]
                .iter()
                .flat_map(|head| {
                    tails.iter().map(move |tail| {
                        [vec![(*head).into(), (*id).into()], tail.clone()].concat()
                    })
                })
                .collect()
        }
        _ => vec![own(parts)],
    }
}

/// Fold every spelling in `document` into the typed layout; a canonical
/// spelling wins over a shorthand of the same key.
pub fn fold(document: &mut Value) {
    let Some(map) = document.as_mapping_mut() else {
        return;
    };
    // `repo:` also keys a settings recovery file by repository path; only its
    // `plan` group is a setting.
    let repo_plan = map
        .get_mut("repo")
        .and_then(Value::as_mapping_mut)
        .and_then(|repo| repo.remove("plan"));
    if map
        .get("repo")
        .is_some_and(|repo| repo.as_mapping().is_some_and(Mapping::is_empty))
    {
        map.remove("repo");
    }
    // The canonical spelling goes last, so it wins over its shorthand.
    if let Some(plan) = map.remove("plan") {
        merge(map, "planning".into(), plan);
    }
    if let Some(plan) = repo_plan {
        merge(map, "planning".into(), plan);
    }
    // The released `linear: {team}` block; a Wave file's `linear: {initiative}` stays.
    if let Some(team) = map
        .get_mut("linear")
        .and_then(Value::as_mapping_mut)
        .and_then(|linear| linear.remove("team"))
    {
        if let Some(planning) = map.get_mut("planning").and_then(Value::as_mapping_mut) {
            if planning.get("provider").and_then(Value::as_str) == Some("linear")
                && !planning.contains_key("team")
            {
                planning.insert("team".into(), team);
            }
        }
    }
    if map
        .get("linear")
        .is_some_and(|linear| linear.as_mapping().is_some_and(Mapping::is_empty))
    {
        map.remove("linear");
    }
    if let Some(skill) = map.remove("skill") {
        let mut merged = map.remove("skills").unwrap_or(Value::Null);
        super::files::overlay(&mut merged, &skill);
        map.insert("skills".into(), merged);
    }
    if let Some(Value::Mapping(wave)) = map.remove("wave") {
        for (key, value) in wave {
            if key.as_str() == Some("graph") {
                map.insert("graph".into(), value);
            } else {
                let mut waves = map.remove("waves").unwrap_or(Value::Null);
                if !waves.is_mapping() {
                    waves = Value::Mapping(Mapping::new());
                }
                merge(
                    waves.as_mapping_mut().expect("mapping initialized"),
                    key,
                    value,
                );
                map.insert("waves".into(), waves);
            }
        }
    }
    if let Some(waves) = map.get_mut("waves").and_then(Value::as_mapping_mut) {
        for value in waves.values_mut() {
            fold(value);
        }
    }
}

// Insert `value` under `key`, overlaying what a shorthand already declared.
fn merge(map: &mut Mapping, key: Value, value: Value) {
    match map.get_mut(&key) {
        Some(existing) => super::files::overlay(existing, &value),
        None => {
            map.insert(key, value);
        }
    }
}

/// Fold every spelling of the group holding the typed path `parts` into the
/// released layout in `document`, and return the path to edit. Released `lf`
/// silently ignores `repo:`, `skill:` and `wave:` in a committed file, so
/// writers keep the layout both builds read until a release reads the new one
/// (Task 3644a6 moves them to the canonical spellings).
/// Groups the path does not touch keep their spelling.
pub fn gather(document: &mut Value, parts: &[&str], root: Root) -> Vec<String> {
    let (group, rest, inner): (&[&str], &[&str], Root) = match parts {
        ["planning", rest @ ..] => (&parts[..1], rest, Root::General),
        ["skills", _, rest @ ..] => (&parts[..2], rest, Root::General),
        ["graph"] if root == Root::General => (parts, &[], Root::General),
        ["waves", _, rest @ ..] => (&parts[..2], rest, Root::Wave),
        _ => return parts.iter().map(|part| (*part).to_owned()).collect(),
    };
    let target: Vec<String> = group.iter().map(|part| (*part).to_owned()).collect();
    // Gather the group's spellings into their own document and fold it, so
    // the same rule as a read decides what the group holds.
    let mut spelled = spellings(group, root);
    if group == ["planning"] {
        spelled.push(vec!["linear".into()]);
    }
    let mut partial = Value::Mapping(Mapping::new());
    for spelling in spelled {
        let path: Vec<&str> = spelling.iter().map(String::as_str).collect();
        if let Some(found) = path
            .iter()
            .try_fold(&*document, |value, key| value.get(*key))
            .cloned()
        {
            super::settings::insert(&mut partial, &path, found);
            super::files::remove(document, &path);
        }
    }
    fold(&mut partial);
    let gathered = group
        .iter()
        .try_fold(&partial, |value, key| value.get(*key))
        .cloned()
        .unwrap_or(Value::Null);
    if !gathered.is_null() {
        super::settings::insert(
            document,
            &target.iter().map(String::as_str).collect::<Vec<_>>(),
            gathered,
        );
    }
    if rest.is_empty() {
        return target;
    }
    let node = target
        .iter()
        .try_fold(&mut *document, |value, key| value.get_mut(key.as_str()));
    let tail = match node {
        Some(node) => gather(node, rest, inner),
        None => rest.iter().map(|part| (*part).to_owned()).collect(),
    };
    [target, tail].concat()
}

/// A definition's canonical key and the other spellings `typed` accepts for it.
pub fn shorthands(key: &str) -> Vec<String> {
    let canonical = canonical_key(key);
    let mut shorthands = Vec::new();
    if canonical != key {
        shorthands.push(key.to_owned());
    }
    if key == "planning" {
        shorthands.push("linear.team".into());
    }
    shorthands
}

#[cfg(test)]
mod tests {
    use super::{canonical, fold, gather, typed, Root};
    use serde_yaml_ng::Value;

    fn yaml(text: &str) -> Value {
        serde_yaml_ng::from_str(text).unwrap()
    }

    #[test]
    fn every_spelling_names_one_typed_path() {
        for (spelled, expected) in [
            ("repo.plan.team", "planning.team"),
            ("plan.team", "planning.team"),
            ("planning.provider", "planning.provider"),
            ("linear.team", "planning.team"),
            ("skill.design.agent", "skills.design.agent"),
            ("skills.design.agent", "skills.design.agent"),
            ("wave.graph", "graph"),
            ("graph", "graph"),
            ("wave.abc.account", "waves.abc.account"),
            (
                "wave.abc.skill.design.agent",
                "waves.abc.skills.design.agent",
            ),
            ("waves.abc.graph", "waves.abc.graph"),
            ("context.docs", "context.docs"),
        ] {
            assert_eq!(super::typed_key(spelled), expected, "{spelled}");
        }
        assert_eq!(
            canonical(&["waves", "abc", "graph"], Root::General),
            ["wave", "abc", "graph"]
        );
        assert_eq!(canonical(&["graph"], Root::Wave), ["graph"]);
        assert_eq!(canonical(&["graph"], Root::General), ["wave", "graph"]);
        assert_eq!(
            typed(&["wave", "abc", "wave", "graph"]),
            ["waves", "abc", "graph"]
        );
    }

    #[test]
    fn folding_reads_shorthand_canonical_and_released_spellings_as_one_document() {
        let mut document = yaml(
            "planning: {provider: linear}\nlinear: {team: released}\nskills: {design: {agent: codex}}\nskill: {design: {agent: claude}, review: {agent: pi}}\nwave:\n  graph: feature\n  abc: {account: {claude: [me]}, skill: {design: {chrome: false}}}\nwaves:\n  abc: {skills: {design: {agent: codex}}}\n",
        );
        fold(&mut document);
        assert_eq!(
            document,
            yaml(
                "planning: {provider: linear, team: released}\nskills: {design: {agent: claude}, review: {agent: pi}}\ngraph: feature\nwaves:\n  abc: {skills: {design: {agent: codex, chrome: false}}, account: {claude: [me]}}\n"
            )
        );
        let mut explicit =
            yaml("repo: {plan: {provider: linear, team: typed}}\nlinear: {team: released}\n");
        fold(&mut explicit);
        assert_eq!(explicit["planning"]["team"], "typed");
    }

    #[test]
    fn writers_gather_only_the_edited_group_in_the_layout_released_lf_reads() {
        let mut document = yaml(
            "repo: {plan: {provider: linear}}\nlinear: {team: released}\nskill: {design: {agent: codex}}\nagent: claude\n",
        );
        let path = gather(&mut document, &["planning", "team"], Root::General);
        assert_eq!(path, ["planning", "team"]);
        assert_eq!(
            document,
            yaml("planning: {provider: linear, team: released}\nskill: {design: {agent: codex}}\nagent: claude\n")
        );
        let path = gather(
            &mut document,
            &["waves", "abc", "skills", "design", "agent"],
            Root::General,
        );
        assert_eq!(path, ["waves", "abc", "skills", "design", "agent"]);
        assert!(document.get("wave").is_none());
        let mut wave_file = yaml("graph: code\nid: abc\n");
        assert_eq!(gather(&mut wave_file, &["graph"], Root::Wave), ["graph"]);
        assert_eq!(wave_file, yaml("graph: code\nid: abc\n"));
    }

    #[test]
    fn the_canonical_spelling_beats_its_shorthand() {
        let mut document = yaml("plan: {team: short}\nrepo: {plan: {team: long}}\n");
        fold(&mut document);
        assert_eq!(document, yaml("planning: {team: long}\n"));
    }
}
