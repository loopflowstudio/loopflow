//! The bounded query syntax compiles to the same authored selections as defaults.
use super::{
    wave_descendants, DashboardId, MemberRef, Selection, SelectionError, SelectionExpression,
    SelectionFilter, SelectionId, SelectionResolution, SelectionState, SelectionWork,
    WorkWorkspaceKind,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

impl Selection {
    /// Identity of the ordered expression, independent of authored occurrence IDs.
    pub fn expression_key(&self) -> String {
        fn strip_ids(value: &mut serde_json::Value) {
            match value {
                serde_json::Value::Object(fields) => {
                    // Only Selection nodes have both fields; Work IDs remain meaningful.
                    if fields.contains_key("expression") {
                        fields.remove("id");
                    }
                    for value in fields.values_mut() {
                        strip_ids(value);
                    }
                }
                serde_json::Value::Array(items) => items.iter_mut().for_each(strip_ids),
                _ => {}
            }
        }
        let mut value = serde_json::to_value(self).expect("Selection is serializable");
        strip_ids(&mut value);
        format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
    }

    /// Retain authored IDs across clause edits and insertion, including repeated clauses.
    pub fn parse(source: &str, previous: Option<&Self>) -> Result<Self, SelectionError> {
        let tokens = tokenize(source)?;
        let mut parser = Parser {
            tokens,
            at: 0,
            depth: 0,
        };
        let parsed = parser.union()?;
        // A stable composition root keeps a lone clause's occurrence unchanged
        // when a sibling is added later.
        let mut selection = if matches!(parsed.expression, SelectionExpression::Compose { .. }) {
            parsed
        } else {
            Parser::node(SelectionExpression::Compose {
                items: vec![parsed],
            })
        };
        if parser.at != parser.tokens.len() {
            return Err(invalid("unexpected closing parenthesis"));
        }
        let mut used = BTreeSet::new();
        if let Some(previous) = previous {
            previous.validate()?;
            let mut available = Vec::new();
            collect(previous, &mut available);
            used.insert(previous.id.clone());
            restore_ids(&mut selection, &available, &mut used);
            selection.id = previous.id.clone();
            restore_edited_ids(&mut selection, &available, &mut used);
        }
        // Unrestored nodes take source-derived IDs after restoration, skipping
        // IDs that earlier edits moved onto other nodes of this same source.
        name_nodes(
            &mut selection,
            &format!("{:x}", Sha256::digest(source.as_bytes())),
            &mut 0,
            &mut used,
        );
        selection.validate()?;
        Ok(selection)
    }

    pub(super) fn expand_filter(
        filter: &SelectionFilter,
        repository: &str,
        work: &[SelectionWork],
        path: &[SelectionId],
        result: &mut SelectionResolution,
    ) -> Result<(), SelectionError> {
        let scoped: Vec<_> = work
            .iter()
            .filter(|item| item.key.repository == repository)
            .collect();
        let target = match filter {
            SelectionFilter::Wave(name) | SelectionFilter::Task(name) => {
                let kind = if matches!(filter, SelectionFilter::Wave(_)) {
                    WorkWorkspaceKind::Wave
                } else {
                    WorkWorkspaceKind::Task
                };
                let exact = scoped
                    .iter()
                    .find(|item| item.key.kind == kind && item.key.id == *name);
                let matches: Vec<_> = if let Some(exact) = exact {
                    vec![*exact]
                } else {
                    scoped
                        .iter()
                        .copied()
                        .filter(|item| {
                            item.key.kind == kind
                                && (item.name.eq_ignore_ascii_case(name)
                                    || (kind == WorkWorkspaceKind::Task
                                        && name.starts_with('#')
                                        && item.name.rsplit('-').next() == Some(&name[1..])))
                        })
                        .collect()
                };
                match matches.as_slice() {
                    [one] => Some(*one),
                    [] => {
                        result
                            .unavailable
                            .push(format!("{kind:?} {name} is unavailable"));
                        return Ok(());
                    }
                    _ => return Err(SelectionError::Ambiguous(format!("{kind:?} {name}"))),
                }
            }
            _ => None,
        };
        let descendants = match (filter, target) {
            (SelectionFilter::Wave(_), Some(wave)) => {
                wave_descendants(repository, &wave.key.id, work)
            }
            _ => BTreeSet::new(),
        };
        for item in scoped {
            let included = match filter {
                SelectionFilter::Task(_) => {
                    Some(item.key == target.expect("Task target resolved").key)
                }
                SelectionFilter::Wave(_) => Some(
                    item.parent_wave
                        .as_ref()
                        .is_some_and(|parent| descendants.contains(parent.as_str())),
                ),
                SelectionFilter::Kind(kind) => Some(&item.key.kind == kind),
                SelectionFilter::State(state) if item.key.kind == WorkWorkspaceKind::Task => {
                    match state {
                        SelectionState::Open => item.open,
                        _ if item.states.contains(state) => Some(true),
                        SelectionState::Available
                        | SelectionState::Busy
                        | SelectionState::Blocked
                            if !item.activity_known =>
                        {
                            None
                        }
                        _ => Some(false),
                    }
                }
                SelectionFilter::State(_) => Some(false),
            };
            match included {
                Some(true) => result.members.push(MemberRef {
                    source: path.to_vec(),
                    work: item.key.clone(),
                }),
                None => result
                    .unavailable
                    .push(format!("Task {} activity is unavailable", item.name)),
                Some(false) => {}
            }
        }
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> SelectionError {
    SelectionError::Query(message.into())
}

fn collect<'a>(selection: &'a Selection, nodes: &mut Vec<&'a Selection>) {
    nodes.push(selection);
    if let SelectionExpression::Compose { items } | SelectionExpression::Intersect { items } =
        &selection.expression
    {
        for item in items {
            collect(item, nodes);
        }
    }
}

fn same_expression(a: &SelectionExpression, b: &SelectionExpression) -> bool {
    match (a, b) {
        (SelectionExpression::Compose { items: a }, SelectionExpression::Compose { items: b })
        | (
            SelectionExpression::Intersect { items: a },
            SelectionExpression::Intersect { items: b },
        ) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| same_expression(&a.expression, &b.expression))
        }
        _ => a == b,
    }
}

fn restore_ids(
    selection: &mut Selection,
    previous: &[&Selection],
    used: &mut BTreeSet<SelectionId>,
) {
    if let Some(prior) = previous.iter().find(|prior| {
        !used.contains(&prior.id) && same_expression(&selection.expression, &prior.expression)
    }) {
        selection.id = prior.id.clone();
        used.insert(prior.id.clone());
    }
    if let SelectionExpression::Compose { items } | SelectionExpression::Intersect { items } =
        &mut selection.expression
    {
        for item in items {
            restore_ids(item, previous, used);
        }
    }
}

fn same_clause(a: &SelectionExpression, b: &SelectionExpression) -> bool {
    match (a, b) {
        (SelectionExpression::Query { .. }, SelectionExpression::Query { .. }) => true,
        (SelectionExpression::Filter { filter: a }, SelectionExpression::Filter { filter: b }) => {
            std::mem::discriminant(a) == std::mem::discriminant(b)
        }
        (SelectionExpression::Intersect { .. }, SelectionExpression::Intersect { .. })
        | (SelectionExpression::Compose { .. }, SelectionExpression::Compose { .. }) => true,
        _ => false,
    }
}

// Match unchanged clauses first, then edits of remaining clauses in authored order.
// A newly inserted sibling cannot take an unchanged clause's identity.
fn restore_edited_ids(
    selection: &mut Selection,
    previous: &[&Selection],
    used: &mut BTreeSet<SelectionId>,
) {
    if selection.id.0.is_empty() {
        if let Some(prior) = previous.iter().find(|prior| {
            !used.contains(&prior.id) && same_clause(&selection.expression, &prior.expression)
        }) {
            selection.id = prior.id.clone();
            used.insert(prior.id.clone());
        }
    }
    if let SelectionExpression::Compose { items } | SelectionExpression::Intersect { items } =
        &mut selection.expression
    {
        for item in items {
            restore_edited_ids(item, previous, used);
        }
    }
}

fn tokenize(source: &str) -> Result<Vec<String>, SelectionError> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    for c in source.chars() {
        if c == '"' {
            quoted = !quoted;
            token.push(c);
        } else if !quoted && (c.is_whitespace() || matches!(c, '(' | ')')) {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
            if !c.is_whitespace() {
                tokens.push(c.to_string());
            }
        } else {
            token.push(c);
        }
    }
    if quoted {
        return Err(invalid("close the quoted value"));
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<String>,
    at: usize,
    depth: usize,
}
impl Parser {
    fn is(&self, token: &str) -> bool {
        self.tokens
            .get(self.at)
            .is_some_and(|next| next.eq_ignore_ascii_case(token))
    }
    fn node(expression: SelectionExpression) -> Selection {
        Selection {
            // Parsing assigns deterministic IDs to the complete tree in name_nodes.
            id: SelectionId(String::new()),
            expression,
        }
    }
    fn union(&mut self) -> Result<Selection, SelectionError> {
        let mut items = vec![self.intersection()?];
        while self.is("or") {
            self.at += 1;
            items.push(self.intersection()?);
        }
        Ok(if items.len() == 1 {
            items.remove(0)
        } else {
            Self::node(SelectionExpression::Compose { items })
        })
    }
    fn intersection(&mut self) -> Result<Selection, SelectionError> {
        let mut items = vec![self.atom()?];
        while self.at < self.tokens.len() && !self.is(")") && !self.is("or") {
            if self.is("and") {
                self.at += 1;
            }
            items.push(self.atom()?);
        }
        Ok(if items.len() == 1 {
            items.remove(0)
        } else {
            Self::node(SelectionExpression::Intersect { items })
        })
    }
    fn atom(&mut self) -> Result<Selection, SelectionError> {
        let token = self
            .tokens
            .get(self.at)
            .cloned()
            .ok_or_else(|| invalid("expected a filter"))?;
        self.at += 1;
        if token == "(" {
            self.depth += 1;
            if self.depth > 64 {
                return Err(invalid("query nesting exceeds 64 groups"));
            }
            let node = self.union()?;
            if !self.is(")") {
                return Err(invalid("close the parenthesis"));
            }
            self.at += 1;
            self.depth -= 1;
            return Ok(node);
        }
        let (key, raw) = token
            .split_once(':')
            .ok_or_else(|| invalid(format!("expected a filter, found {token}")))?;
        let value = if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
            &raw[1..raw.len() - 1]
        } else {
            raw
        };
        if value.is_empty() || value.contains('"') {
            return Err(invalid("expected a nonempty filter value"));
        }
        let filter = match key.to_ascii_lowercase().as_str() {
            "repo" => {
                return Err(invalid(
                    "repository is implicit; select a repository window",
                ))
            }
            "query" => {
                return Ok(Self::node(SelectionExpression::Query {
                    query: DashboardId(value.into()),
                }))
            }
            "task" => SelectionFilter::Task(value.into()),
            "wave" => SelectionFilter::Wave(value.into()),
            "kind" => SelectionFilter::Kind(match value {
                "task" => WorkWorkspaceKind::Task,
                "wave" => WorkWorkspaceKind::Wave,
                _ => return Err(invalid("use kind:task or kind:wave")),
            }),
            "state" => SelectionFilter::State(match value {
                "open" => SelectionState::Open,
                "available" => SelectionState::Available,
                "busy" => SelectionState::Busy,
                "blocked" => SelectionState::Blocked,
                "unstarted" => SelectionState::Unstarted,
                "idle" => SelectionState::Idle,
                "active" => SelectionState::Active,
                "moving" => SelectionState::Moving,
                "shipping" => SelectionState::Shipping,
                "backlog" => SelectionState::Backlog,
                "done" => SelectionState::Done,
                _ => return Err(invalid(format!("unknown state {value}"))),
            }),
            _ => return Err(invalid(format!("unknown filter {key}"))),
        };
        Ok(Self::node(SelectionExpression::Filter { filter }))
    }
}

fn name_nodes(
    selection: &mut Selection,
    source: &str,
    next: &mut usize,
    used: &mut BTreeSet<SelectionId>,
) {
    if selection.id.0.is_empty() {
        let id = loop {
            let id = SelectionId(format!("{source}:{next}"));
            *next += 1;
            if !used.contains(&id) {
                break id;
            }
        };
        used.insert(id.clone());
        selection.id = id;
    }
    if let SelectionExpression::Compose { items } | SelectionExpression::Intersect { items } =
        &mut selection.expression
    {
        for item in items {
            name_nodes(item, source, next, used);
        }
    }
}
