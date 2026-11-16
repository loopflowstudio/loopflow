//! What a pass does with one unsettled effect, one function per kind, and the
//! single lost-reply rule every kind shares. Pure: the drain supplies what the
//! provider holds; nothing here performs I/O.

use serde_json::Value;

use crate::planning::StateHistory;
use crate::sqlite::tracker_effects::{EffectKind, EffectPhase, Settlement, TrackerEffect};
use crate::work::edit::WorkValue;

/// The provider's current copy of an effect's owner, in store words.
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
    /// Whether the state moved since a state effect's base, when the provider
    /// keeps that history. `None`: compare values only.
    pub state_history: Option<StateHistory>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Send,
    /// Wait without a request; the owner's later effects wait too.
    Hold,
    Settle(Settlement),
}

/// What to do with an attempted effect before anything else on its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LostReply {
    /// The send carries our id, so repeating it cannot duplicate.
    Resend,
    Look,
    /// Looked recently; the owner waits for the next window.
    Wait,
}

/// An attempted effect is looked for at most once per `window` seconds; its
/// first look happens on the next pass. Idempotent sends repeat instead.
pub fn lost_reply(effect: &TrackerEffect, idempotent: bool, now: i64, window: i64) -> LostReply {
    if idempotent {
        return LostReply::Resend;
    }
    match effect.looked_at {
        Some(at) if now - at < window => LostReply::Wait,
        _ => LostReply::Look,
    }
}

/// Settle `effect` against what the provider holds (`None`: no such object).
/// `parent_pending` is true while a Project this effect names has no remote id.
pub fn decide(
    effect: &TrackerEffect,
    observed: Option<&Observed>,
    parent_pending: bool,
) -> Decision {
    match effect.kind {
        EffectKind::Create => create(observed, parent_pending),
        EffectKind::Field if parent_pending => Decision::Hold,
        EffectKind::Field => field(effect, observed),
        EffectKind::State => state(effect, observed),
        EffectKind::Order => order(effect, observed),
        EffectKind::Comment => comment(effect, observed),
        EffectKind::Delete => delete(effect, observed),
        EffectKind::Link => link(effect, observed),
    }
}

fn acknowledged(observed: &Observed) -> Decision {
    Decision::Settle(Settlement::Acknowledged {
        revision: observed.revision.clone(),
        remote_id: observed.remote_id.clone(),
    })
}

fn create(observed: Option<&Observed>, parent_pending: bool) -> Decision {
    match observed {
        Some(observed) => acknowledged(observed),
        None if parent_pending => Decision::Hold,
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
/// A sent value whose reply was lost is acknowledged when it is there, sent
/// again when the provider still holds the baseline, and otherwise loses to
/// the provider's newer value, which the store keeps beside it.
fn field(effect: &TrackerEffect, observed: Option<&Observed>) -> Decision {
    let Some(value) = work_value(&effect.value) else {
        return Decision::Settle(Settlement::Retire {
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
        return acknowledged(observed);
    }
    if effect.phase == EffectPhase::Pending {
        return Decision::Send;
    }
    match effect
        .base
        .as_ref()
        .and_then(|base| work_value(&base["value"]))
    {
        Some(base) if !same(&base, remote) => adopt(observed, remote),
        _ => Decision::Send,
    }
}

fn adopt(observed: &Observed, remote: &WorkValue) -> Decision {
    Decision::Settle(Settlement::Adopt {
        remote: serde_json::json!({"revision": observed.revision, "value": remote}),
    })
}

/// State is a field, except that a provider keeping state history can say
/// whether a lost write landed while the state now reads as its base: no
/// history holds, an unchanged history re-sends, a changed one adopts.
fn state(effect: &TrackerEffect, observed: Option<&Observed>) -> Decision {
    let history = observed.and_then(|observed| observed.state_history);
    let (Some(observed), Some(history), EffectPhase::Attempted) = (observed, history, effect.phase)
    else {
        return field(effect, observed);
    };
    let Some(remote) = observed
        .values
        .iter()
        .find(|v| matches!(v, WorkValue::Disposition(_)))
    else {
        return field(effect, Some(observed));
    };
    let at_base = effect
        .base
        .as_ref()
        .and_then(|base| work_value(&base["value"]))
        .is_some_and(|base| same(&base, remote));
    if !at_base {
        return field(effect, Some(observed));
    }
    match history {
        StateHistory::Unknown => Decision::Hold,
        StateHistory::Unchanged => Decision::Send,
        StateHistory::Changed => adopt(observed, remote),
    }
}

/// Only a complete list says where Tasks are; without one nothing is sent.
fn order(effect: &TrackerEffect, observed: Option<&Observed>) -> Decision {
    let Some(observed) = observed else {
        return Decision::Hold;
    };
    let Some(order) = &observed.order else {
        return Decision::Hold;
    };
    let desired: Vec<String> = serde_json::from_value(effect.value.clone()).unwrap_or_default();
    if desired.iter().all(|id| order.contains(id)) && same_order(&desired, order) {
        return acknowledged(observed);
    }
    if effect.phase == EffectPhase::Pending {
        return Decision::Send;
    }
    let base: Option<Vec<String>> = effect
        .base
        .as_ref()
        .and_then(|base| serde_json::from_value(base["value"].clone()).ok());
    match base {
        Some(base) if !same_order(&base, order) => Decision::Settle(Settlement::Adopt {
            remote: serde_json::json!({"revision": observed.revision, "value": order}),
        }),
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

fn comment(effect: &TrackerEffect, observed: Option<&Observed>) -> Decision {
    let body = effect.value["value"]["body"].as_str();
    match observed {
        Some(observed)
            if observed.comments.iter().any(|seen| {
                Some(seen.as_str()) == effect.subject.as_deref() || Some(seen.as_str()) == body
            }) =>
        {
            acknowledged(observed)
        }
        _ => Decision::Send,
    }
}

/// Removal settles on a trashed or missing object. An object active at a
/// revision newer than the one the deletion saw was restored by someone else,
/// and that wins.
fn delete(effect: &TrackerEffect, observed: Option<&Observed>) -> Decision {
    let Some(observed) = observed else {
        return Decision::Settle(Settlement::Acknowledged {
            revision: None,
            remote_id: None,
        });
    };
    if observed.deleted {
        return acknowledged(observed);
    }
    let nanos = |revision: Option<&str>| crate::planning::revision_nanos(revision).ok().flatten();
    let base = effect
        .base
        .as_ref()
        .and_then(|base| base["revision"].as_str());
    let newer = match (nanos(base), nanos(observed.revision.as_deref())) {
        (Some(base), Some(seen)) => seen > base,
        _ => false,
    };
    if newer && effect.phase == EffectPhase::Attempted {
        Decision::Settle(Settlement::Retire {
            reason: "restored in the tracker after this deletion".into(),
        })
    } else {
        Decision::Send
    }
}

fn link(effect: &TrackerEffect, observed: Option<&Observed>) -> Decision {
    let wanted: Vec<String> =
        serde_json::from_value(effect.value["value"].clone()).unwrap_or_default();
    match observed {
        Some(observed) if wanted.iter().all(|id| observed.initiatives.contains(id)) => {
            acknowledged(observed)
        }
        _ => Decision::Send,
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, lost_reply, Decision, LostReply, Observed};
    use crate::planning::StateHistory;
    use crate::provider::Provider;
    use crate::sqlite::tracker_effects::{
        EffectKind, EffectOwner, EffectPhase, Settlement, TrackerEffect,
    };
    use crate::work::edit::{Disposition, WorkValue};
    use loopflow_model::durable::TaskId;
    use serde_json::json;

    fn effect(kind: EffectKind, phase: EffectPhase, value: serde_json::Value) -> TrackerEffect {
        TrackerEffect {
            seq: 1,
            id: "effect".into(),
            owner: EffectOwner::Task(TaskId::new()),
            subject: None,
            kind,
            value,
            base: None,
            conflict: None,
            phase,
            provider: Provider::Linear,
            remote_id: None,
            acknowledged_revision: None,
            error: None,
            attempted_at: None,
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
        let mut sent = effect(EffectKind::Field, EffectPhase::Attempted, json!(null));
        assert_eq!(lost_reply(&sent, false, 1_000, 600), LostReply::Look);
        sent.looked_at = Some(900);
        assert_eq!(lost_reply(&sent, false, 1_000, 600), LostReply::Wait);
        assert_eq!(lost_reply(&sent, false, 1_500, 600), LostReply::Look);
        assert_eq!(lost_reply(&sent, true, 1_000, 600), LostReply::Resend);
    }

    #[test]
    fn a_lost_field_reply_settles_on_what_the_provider_holds() {
        let mut sent = effect(
            EffectKind::Field,
            EffectPhase::Attempted,
            serde_json::to_value(title("Mine")).unwrap(),
        );
        sent.base = Some(json!({"revision":"2026-11-01T00:00:00Z","value":title("Before")}));
        assert!(matches!(
            decide(&sent, Some(&holding(vec![title("Mine")], "r2")), false),
            Decision::Settle(Settlement::Acknowledged { .. })
        ));
        assert_eq!(
            decide(&sent, Some(&holding(vec![title("Before")], "r1")), false),
            Decision::Send,
            "the provider never saw it"
        );
        assert_eq!(
            decide(&sent, Some(&holding(vec![title("Theirs")], "r3")), false),
            Decision::Settle(Settlement::Adopt {
                remote: json!({"revision":"r3","value":title("Theirs")})
            }),
            "a coworker's newer value wins and the effect keeps ours"
        );
        sent.phase = EffectPhase::Pending;
        assert_eq!(
            decide(&sent, Some(&holding(vec![title("Theirs")], "r3")), false),
            Decision::Send,
            "an unsent save goes out over a provider edit"
        );
    }

    #[test]
    fn a_deletion_restored_later_in_the_tracker_retires() {
        let mut removal = effect(
            EffectKind::Delete,
            EffectPhase::Attempted,
            json!({"value":true}),
        );
        removal.base = Some(json!({"revision":"2026-11-01T00:00:00Z","value":false}));
        let active = holding(vec![], "2026-11-02T00:00:00Z");
        assert!(matches!(
            decide(&removal, Some(&active), false),
            Decision::Settle(Settlement::Retire { .. })
        ));
        let trashed = Observed {
            deleted: true,
            ..active.clone()
        };
        assert!(matches!(
            decide(&removal, Some(&trashed), false),
            Decision::Settle(Settlement::Acknowledged { .. })
        ));
        assert!(matches!(
            decide(&removal, None, false),
            Decision::Settle(Settlement::Acknowledged { .. })
        ));
        let unchanged = holding(vec![], "2026-11-01T00:00:00Z");
        assert_eq!(decide(&removal, Some(&unchanged), false), Decision::Send);
    }

    #[test]
    fn order_moves_only_on_complete_list_evidence() {
        let sent = effect(EffectKind::Order, EffectPhase::Attempted, json!(["a", "b"]));
        let partial = holding(vec![], "r1");
        assert_eq!(decide(&sent, Some(&partial), false), Decision::Hold);
        let done = Observed {
            order: Some(vec!["a".into(), "b".into()]),
            ..partial.clone()
        };
        assert!(matches!(
            decide(&sent, Some(&done), false),
            Decision::Settle(Settlement::Acknowledged { .. })
        ));
        let other = Observed {
            order: Some(vec!["b".into(), "a".into()]),
            ..partial
        };
        assert_eq!(decide(&sent, Some(&other), false), Decision::Send);
    }

    #[test]
    fn a_task_waits_for_its_projects_creation() {
        let create = effect(EffectKind::Create, EffectPhase::Pending, json!([]));
        assert_eq!(decide(&create, None, true), Decision::Hold);
        assert_eq!(decide(&create, None, false), Decision::Send);
    }

    #[test]
    fn a_comment_settles_on_its_id_or_its_visible_text() {
        let mut note = effect(
            EffectKind::Comment,
            EffectPhase::Attempted,
            json!({"field":"comment_content","value":{"body":"first comment"}}),
        );
        note.subject = Some("comment-1".into());
        let by_text = Observed {
            comments: vec!["first comment".into()],
            ..Observed::default()
        };
        assert!(matches!(
            decide(&note, Some(&by_text), false),
            Decision::Settle(Settlement::Acknowledged { .. })
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
        let mut sent = effect(
            EffectKind::State,
            EffectPhase::Attempted,
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
            Decision::Settle(Settlement::Adopt { .. })
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
