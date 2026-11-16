//! A tracker bridge carries the store's queued effects out and answers, in
//! store words, what the provider holds. The drain owns pacing; per-kind rules
//! own settlement.

pub mod drain;
pub mod rules;

use loopflow_storage::provider::Provider;
use loopflow_storage::sqlite::tracker_effects::{EffectKind, TrackerEffect};

use rules::Observed;

/// What one send did.
#[derive(Debug, Clone, PartialEq)]
pub enum Sent {
    Acknowledged {
        revision: Option<String>,
        remote_id: Option<String>,
    },
    /// Refused before acting (validation, rate limit): safe to send again.
    Rejected(String),
    /// No reply: the provider may or may not have acted.
    Unknown(String),
}

/// The configured bridge. Linear and Asana arms land with their `effects.rs`.
#[derive(Debug)]
#[non_exhaustive]
pub enum Tracker {
    #[cfg(test)]
    Fake(std::sync::Arc<fake::Fake>),
}

// Until the Linear and Asana arms land, release builds have no arm to use these.
#[cfg_attr(not(test), allow(unused_variables))]
impl Tracker {
    pub fn provider(&self) -> Provider {
        match *self {
            #[cfg(test)]
            Self::Fake(ref fake) => fake.provider,
        }
    }

    /// A send carrying our own id cannot duplicate, so a lost reply repeats it.
    pub fn idempotent(&self, kind: EffectKind) -> bool {
        match *self {
            #[cfg(test)]
            Self::Fake(ref fake) => fake.idempotent.contains(&kind),
        }
    }

    pub async fn send(&self, effect: &TrackerEffect) -> Sent {
        match *self {
            #[cfg(test)]
            Self::Fake(ref fake) => fake.send(effect),
        }
    }

    /// Read back the owner an attempted effect addresses. `Ok(None)`: the
    /// provider has no such object.
    pub async fn look(&self, effect: &TrackerEffect) -> Result<Option<Observed>, String> {
        match *self {
            #[cfg(test)]
            Self::Fake(ref fake) => fake.look(effect),
        }
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use loopflow_storage::provider::Provider;
    use loopflow_storage::sqlite::tracker_effects::{EffectKind, TrackerEffect};

    use super::rules::Observed;
    use super::Sent;

    /// A provider that acknowledges every send and answers looks from `held`.
    #[derive(Debug)]
    pub struct Fake {
        pub provider: Provider,
        pub idempotent: Vec<EffectKind>,
        pub held: Mutex<Option<Observed>>,
        pub requests: AtomicUsize,
    }

    impl Fake {
        pub fn new(provider: Provider) -> Self {
            Self {
                provider,
                idempotent: Vec::new(),
                held: Mutex::new(None),
                requests: AtomicUsize::new(0),
            }
        }

        pub fn requests(&self) -> usize {
            self.requests.load(Ordering::SeqCst)
        }

        pub(super) fn send(&self, effect: &TrackerEffect) -> Sent {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Sent::Acknowledged {
                revision: Some("2026-11-16T00:00:00Z".into()),
                remote_id: Some(format!("remote-{}", effect.id)),
            }
        }

        pub(super) fn look(&self, _effect: &TrackerEffect) -> Result<Option<Observed>, String> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Ok(self.held.lock().unwrap().clone())
        }
    }
}
