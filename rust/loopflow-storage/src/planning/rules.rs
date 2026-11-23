//! What a pass does with one unconfirmed action, one function per kind, and the
//! single lost-reply rule every kind shares. Pure: the drain supplies what the
//! provider holds; nothing here performs I/O.

use serde_json::Value;

use crate::planning::StateHistory;
use crate::sqlite::tracker_actions::{ActionKind, ActionPhase, Resolution, TrackerAction};
use crate::work::edit::WorkValue;

/// The provider's current copy of an action's owner, in store words.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Observed {
    pub remote_id: Option<String>,
    pub revision: Option<String>,
    /// Field values, `Disposition` included.
    pub values: Vec<WorkValue>,
    /// The owner's complete child order, when the provider answered with all of it.
    pub order: Option<Vec<String>>,
    /// Comment ids, or visible text where the provider keeps no id of ours.
    pub comments: Vec<String>,
    pub initiatives: Vec<String>,
    pub deleted: bool,
    /// Whether the state moved since a state action's base, when the provider
    /// keeps that history. `None`: compare values only.
    pub state_history: Option<StateHistory>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Send,
    /// Wait without a request; the owner's later actions wait too.
    Hold,
    Resolve(Resolution),
}

/// What to do with a sent action before anything else on its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LostReply {
    /// The send carries our id, so repeating it cannot duplicate.
    Resend,
    Look,
    /// Looked recently; the owner waits for the next window.
    Wait,
}

/// A sent action is looked for at most once per `window` seconds; its
/// first look happens on the next pass. Idempotent sends repeat instead, at
/// most once per `window` after the last send, so one that keeps failing
/// does not go out again from every pass.
pub fn lost_reply(action: &TrackerAction, idempotent: bool, now: i64, window: i64) -> LostReply {
    if idempotent {
        return match action.sent_at {
            Some(at) if now - at < window => LostReply::Wait,
            _ => LostReply::Resend,
        };
    }
    match action.looked_at {
        Some(at) if now - at < window => LostReply::Wait,
        _ => LostReply::Look,
    }
}

/// Resolve `action` against what the provider holds (`None`: no such object).
/// `parent_uncreated` is true while a Project this action names has no remote id.
pub fn decide(
    action: &TrackerAction,
    observed: Option<&Observed>,
    parent_uncreated: bool,
) -> Decision {
    match action.kind {
        ActionKind::Create => create(observed, parent_uncreated),
        ActionKind::Field if parent_uncreated => Decision::Hold,
        ActionKind::Field => field(action, observed),
        ActionKind::State => state(action, observed),
        ActionKind::Order => order(action, observed),
        ActionKind::Comment => comment(action, observed),
        ActionKind::Delete => delete(action, observed),
        ActionKind::Link => link(action, observed),
    }
}

fn confirmed(observed: &Observed) -> Decision {
    Decision::Resolve(Resolution::Confirmed {
        revision: observed.revision.clone(),
        remote_id: observed.remote_id.clone(),
    })
}

fn create(observed: Option<&Observed>, parent_uncreated: bool) -> Decision {
    match observed {
        Some(observed) => confirmed(observed),
        None if parent_uncreated => Decision::Hold,
        None => Decision::Send,
    }
}

fn work_value(value: &Value) -> Option<WorkValue> {
    serde_json::from_value(value.clone()).ok()
}

// State compares the state and completion, never the completion time a
// provider stamps itself.
fn same(a: &WorkValue, b: &WorkValue) -> bool {
    match (a, b) {
        (WorkValue::Disposition(a), WorkValue::Disposition(b)) => {
            a.planning_state == b.planning_state && a.planning_completed == b.planning_completed
        }
        _ => a == b,
    }
}

/// Unsent values go out even over a provider edit: the change sent last wins.
/// A sent value whose reply was lost is confirmed when it is there, sent
/// again when the provider still holds the baseline, and otherwise loses to
/// the provider's newer value, which the store keeps beside it.
fn field(action: &TrackerAction, observed: Option<&Observed>) -> Decision {
    let Some(value) = work_value(&action.value) else {
        return Decision::Resolve(Resolution::Retire {
            reason: "unreadable saved value".into(),
        });
    };
    let Some(observed) = observed else {
        return Decision::Send;
    };
    let Some(remote) = observed.values.iter().find(|v| v.field() == value.field()) else {
        return Decision::Send;
    };
    if same(remote, &value) {
        return confirmed(observed);
    }
    if action.phase == ActionPhase::Queued {
        return Decision::Send;
    }
    match action
        .base
        .as_ref()
        .and_then(|base| work_value(&base["value"]))
    {
        Some(base) if !same(&base, remote) => adopt(observed, remote),
        _ => Decision::Send,
    }
}

fn adopt(observed: &Observed, remote: impl serde::Serialize) -> Decision {
    Decision::Resolve(Resolution::Conflicted {
        remote: serde_json::json!({"revision": observed.revision, "value": remote}),
    })
}

/// State is a field, except that a provider keeping state history can say
/// whether a lost write landed while the state now reads as its base: no
/// history holds, an unchanged history re-sends, a changed one adopts.
fn state(action: &TrackerAction, observed: Option<&Observed>) -> Decision {
    let history = observed.and_then(|observed| observed.state_history);
    let (Some(observed), Some(history), ActionPhase::Sent) = (observed, history, action.phase)
    else {
        return field(action, observed);
    };
    let Some(remote) = observed
        .values
        .iter()
        .find(|v| matches!(v, WorkValue::Disposition(_)))
    else {
        return field(action, Some(observed));
    };
    let at_base = action
        .base
        .as_ref()
        .and_then(|base| work_value(&base["value"]))
        .is_some_and(|base| same(&base, remote));
    if !at_base {
        return field(action, Some(observed));
    }
    match history {
        StateHistory::Unknown => Decision::Hold,
        StateHistory::Unchanged => Decision::Send,
        StateHistory::Changed => adopt(observed, remote),
    }
}

/// Only a complete list says where Tasks are; without one nothing is sent.
fn order(action: &TrackerAction, observed: Option<&Observed>) -> Decision {
    let Some(observed) = observed else {
        return Decision::Hold;
    };
    let Some(order) = &observed.order else {
        return Decision::Hold;
    };
    let desired: Vec<String> = serde_json::from_value(action.value.clone()).unwrap_or_default();
    if desired.iter().all(|id| order.contains(id)) && same_order(&desired, order) {
        return confirmed(observed);
    }
    if action.phase == ActionPhase::Queued {
        return Decision::Send;
    }
    let base: Option<Vec<String>> = action
        .base
        .as_ref()
        .and_then(|base| serde_json::from_value(base["value"].clone()).ok());
    match base {
        Some(base) if !same_order(&base, order) => adopt(observed, order),
        _ => Decision::Send,
    }
}

/// Relative order among shared members. A new member does not conflict with
/// a saved move; a missing one is incomplete evidence, not a move.
pub fn same_order(left: &[String], right: &[String]) -> bool {
    left.iter()
        .filter(|id| right.contains(id))
        .eq(right.iter().filter(|id| left.contains(id)))
}

fn comment(action: &TrackerAction, observed: Option<&Observed>) -> Decision {
    let body = action.comment_body();
    match observed {
        Some(observed)
            if observed.comments.iter().any(|seen| {
                Some(seen.as_str()) == action.subject.as_deref() || Some(seen.as_str()) == body
            }) =>
        {
            confirmed(observed)
        }
        _ => Decision::Send,
    }
}

/// Removal is confirmed by a trashed or missing object. An object active at a
/// revision newer than the one the deletion saw was restored by someone else,
/// and that wins.
fn delete(action: &TrackerAction, observed: Option<&Observed>) -> Decision {
    let Some(observed) = observed else {
        return Decision::Resolve(Resolution::Confirmed {
            revision: None,
            remote_id: None,
        });
    };
    if observed.deleted {
        return confirmed(observed);
    }
    let nanos = |revision: Option<&str>| crate::planning::revision_nanos(revision).ok().flatten();
    let base = action
        .base
        .as_ref()
        .and_then(|base| base["revision"].as_str());
    let newer = match (nanos(base), nanos(observed.revision.as_deref())) {
        (Some(base), Some(seen)) => seen > base,
        _ => false,
    };
    if newer && action.phase == ActionPhase::Sent {
        Decision::Resolve(Resolution::Retire {
            reason: "restored in the tracker after this deletion".into(),
        })
    } else {
        Decision::Send
    }
}

fn link(action: &TrackerAction, observed: Option<&Observed>) -> Decision {
    let wanted: Vec<String> =
        serde_json::from_value(action.value["value"].clone()).unwrap_or_default();
    match observed {
        Some(observed) if wanted.iter().all(|id| observed.initiatives.contains(id)) => {
            confirmed(observed)
        }
        _ => Decision::Send,
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, lost_reply, Decision, LostReply, Observed};
    use crate::planning::StateHistory;
    use crate::planning_sync::TrackerKind;
    use crate::sqlite::tracker_actions::{
        ActionKind, ActionOwner, ActionPhase, Resolution, TrackerAction,
    };
    use crate::work::edit::{Disposition, WorkValue};
    use loopflow_model::durable::TaskId;
    use serde_json::json;

    fn action(kind: ActionKind, phase: ActionPhase, value: serde_json::Value) -> TrackerAction {
        TrackerAction {
            seq: 1,
            id: "action".into(),
            owner: ActionOwner::Task(TaskId::new()),
            subject: None,
            kind,
            value,
            base: None,
            conflict: None,
            phase,
            tracker: TrackerKind::Linear,
            remote_id: None,
            confirmed_revision: None,
            error: None,
            sent_at: None,
            looked_at: None,
        }
    }

    fn title(value: &str) -> WorkValue {
        WorkValue::TaskTitle(Some(value.into()))
    }

    fn holding(values: Vec<WorkValue>, revision: &str) -> Observed {
        Observed {
            revision: Some(revision.into()),
            values,
            ..Observed::default()
        }
    }

    #[test]
    fn a_lost_reply_is_looked_for_once_per_window() {
        let mut sent = action(ActionKind::Field, ActionPhase::Sent, json!(null));
        assert_eq!(lost_reply(&sent, false, 1_000, 600), LostReply::Look);
        sent.looked_at = Some(900);
        assert_eq!(lost_reply(&sent, false, 1_000, 600), LostReply::Wait);
        assert_eq!(lost_reply(&sent, false, 1_500, 600), LostReply::Look);
    }

    #[test]
    fn an_idempotent_send_repeats_once_per_window() {
        let mut sent = action(ActionKind::Link, ActionPhase::Sent, json!(null));
        assert_eq!(lost_reply(&sent, true, 1_000, 600), LostReply::Resend);
        sent.sent_at = Some(900);
        assert_eq!(lost_reply(&sent, true, 1_000, 600), LostReply::Wait);
        assert_eq!(lost_reply(&sent, true, 1_500, 600), LostReply::Resend);
        // `lf task sync` sends now.
        assert_eq!(lost_reply(&sent, true, 1_000, 0), LostReply::Resend);
    }

    #[test]
    fn a_lost_field_reply_resolves_on_what_the_provider_holds() {
        let mut sent = action(
            ActionKind::Field,
            ActionPhase::Sent,
            serde_json::to_value(title("Mine")).unwrap(),
        );
        sent.base = Some(json!({"revision":"2026-11-01T00:00:00Z","value":title("Before")}));
        assert!(matches!(
            decide(&sent, Some(&holding(vec![title("Mine")], "r2")), false),
            Decision::Resolve(Resolution::Confirmed { .. })
        ));
        assert_eq!(
            decide(&sent, Some(&holding(vec![title("Before")], "r1")), false),
            Decision::Send,
            "the provider never saw it"
        );
        assert_eq!(
            decide(&sent, Some(&holding(vec![title("Theirs")], "r3")), false),
            Decision::Resolve(Resolution::Conflicted {
                remote: json!({"revision":"r3","value":title("Theirs")})
            }),
            "a coworker's newer value wins and the action keeps ours"
        );
        sent.phase = ActionPhase::Queued;
        assert_eq!(
            decide(&sent, Some(&holding(vec![title("Theirs")], "r3")), false),
            Decision::Send,
            "an unsent save goes out over a provider edit"
        );
    }

    #[test]
    fn a_deletion_restored_later_in_the_tracker_retires() {
        let mut removal = action(ActionKind::Delete, ActionPhase::Sent, json!({"value":true}));
        removal.base = Some(json!({"revision":"2026-11-01T00:00:00Z","value":false}));
        let active = holding(vec![], "2026-11-02T00:00:00Z");
        assert!(matches!(
            decide(&removal, Some(&active), false),
            Decision::Resolve(Resolution::Retire { .. })
        ));
        let trashed = Observed {
            deleted: true,
            ..active.clone()
        };
        assert!(matches!(
            decide(&removal, Some(&trashed), false),
            Decision::Resolve(Resolution::Confirmed { .. })
        ));
        assert!(matches!(
            decide(&removal, None, false),
            Decision::Resolve(Resolution::Confirmed { .. })
        ));
        let unchanged = holding(vec![], "2026-11-01T00:00:00Z");
        assert_eq!(decide(&removal, Some(&unchanged), false), Decision::Send);
    }

    #[test]
    fn order_moves_only_on_complete_list_evidence() {
        let sent = action(ActionKind::Order, ActionPhase::Sent, json!(["a", "b"]));
        let partial = holding(vec![], "r1");
        assert_eq!(decide(&sent, Some(&partial), false), Decision::Hold);
        let done = Observed {
            order: Some(vec!["a".into(), "b".into()]),
            ..partial.clone()
        };
        assert!(matches!(
            decide(&sent, Some(&done), false),
            Decision::Resolve(Resolution::Confirmed { .. })
        ));
        let other = Observed {
            order: Some(vec!["b".into(), "a".into()]),
            ..partial
        };
        assert_eq!(decide(&sent, Some(&other), false), Decision::Send);
    }

    #[test]
    fn a_task_waits_for_its_projects_creation() {
        let create = action(ActionKind::Create, ActionPhase::Queued, json!([]));
        assert_eq!(decide(&create, None, true), Decision::Hold);
        assert_eq!(decide(&create, None, false), Decision::Send);
    }

    #[test]
    fn a_comment_resolves_on_its_id_or_its_visible_text() {
        let mut note = action(
            ActionKind::Comment,
            ActionPhase::Sent,
            json!({"field":"comment_content","value":{"body":"first comment"}}),
        );
        note.subject = Some("comment-1".into());
        let by_text = Observed {
            comments: vec!["first comment".into()],
            ..Observed::default()
        };
        assert!(matches!(
            decide(&note, Some(&by_text), false),
            Decision::Resolve(Resolution::Confirmed { .. })
        ));
        assert_eq!(
            decide(&note, Some(&Observed::default()), false),
            Decision::Send
        );
    }

    fn disposition(state: &str) -> WorkValue {
        WorkValue::Disposition(Disposition {
            planning_state: Some(state.into()),
            planning_completed: i64::from(state == "completed"),
            planning_completed_at: None,
        })
    }

    #[test]
    fn a_lost_state_reply_at_its_base_follows_the_provider_history() {
        let mut sent = action(
            ActionKind::State,
            ActionPhase::Sent,
            serde_json::to_value(disposition("completed")).unwrap(),
        );
        sent.base = Some(json!({"revision":"r1","value":disposition("unstarted")}));
        let at_base = |history| Observed {
            state_history: Some(history),
            ..holding(vec![disposition("unstarted")], "r2")
        };
        assert_eq!(
            decide(&sent, Some(&at_base(StateHistory::Unknown)), false),
            Decision::Hold
        );
        assert_eq!(
            decide(&sent, Some(&at_base(StateHistory::Unchanged)), false),
            Decision::Send
        );
        assert!(matches!(
            decide(&sent, Some(&at_base(StateHistory::Changed)), false),
            Decision::Resolve(Resolution::Conflicted { .. })
        ));
        assert_eq!(
            decide(
                &sent,
                Some(&holding(vec![disposition("unstarted")], "r2")),
                false
            ),
            Decision::Send,
            "a provider without history compares values"
        );
    }
}
