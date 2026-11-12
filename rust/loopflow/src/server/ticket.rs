//! Tickets admit a second connection for what the control connection must not
//! carry: a terminal's bytes, or one message larger than the control limit.
//! A ticket is single use, bound to the admission that requested it, and
//! expires unused, by wall-clock time so host sleep counts toward its lifetime.
//! It grants nothing the control connection did not already have.
use super::outbox::Reservation;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use base64::Engine as _;
use tokio::sync::mpsc;

use super::wire::ClientMessage;
use crate::transport::Host;

/// How long an unused ticket stays redeemable.
pub use super::wire::TICKET_LIFETIME as TICKET_TTL;

/// Who a connection was admitted as; a ticket is redeemable by the same party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Admission {
    /// The Machine's own user over the private socket.
    Local,
    /// The owner's other Tailscale device, by node and user.
    Remote { node: String, user: u64 },
}

/// What a ticketed connection carries once admitted.
#[derive(Debug)]
pub(super) enum Purpose {
    /// The viewer protocol of an owner or a held terminal, copied byte for byte.
    Terminal(Host),
    /// One server message too large for the control connection.
    Download {
        message: String,
        reservation: Arc<Reservation>,
    },
    /// One client request too large for the control connection; it joins the
    /// issuing session's requests.
    Upload {
        requests: mpsc::Sender<(ClientMessage, Arc<Reservation>)>,
        bytes: usize,
        reservation: Arc<Reservation>,
    },
}

#[derive(Debug)]
struct Ticket {
    admission: Admission,
    purpose: Purpose,
    expires: SystemTime,
}

/// Every ticket the server has issued and not yet redeemed or expired.
#[derive(Debug, Default)]
pub(super) struct Tickets(Mutex<HashMap<String, Ticket>>);

impl Tickets {
    pub(super) fn issue(&self, admission: Admission, purpose: Purpose) -> String {
        self.issue_at(admission, purpose, SystemTime::now())
    }

    fn issue_at(&self, admission: Admission, purpose: Purpose, now: SystemTime) -> String {
        let ticket = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(uuid::Uuid::new_v4().as_bytes());
        let mut issued = self.0.lock().expect("ticket store is never poisoned");
        issued.retain(|_, ticket| ticket.expires > now);
        issued.insert(
            ticket.clone(),
            Ticket {
                admission,
                purpose,
                expires: now + TICKET_TTL,
            },
        );
        ticket
    }

    pub(super) async fn expiring(&self) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            self.0
                .lock()
                .expect("ticket store is never poisoned")
                .retain(|_, ticket| ticket.expires > SystemTime::now());
        }
    }

    /// The ticket's purpose, once; `None` for an unknown, expired, or foreign
    /// ticket. Any attempt spends a terminal ticket; a bulk ticket stays
    /// redeemable by its own device after another's attempt.
    pub(super) fn redeem(&self, ticket: &str, admission: &Admission) -> Option<Purpose> {
        self.redeem_at(ticket, admission, SystemTime::now())
    }

    fn redeem_at(&self, ticket: &str, admission: &Admission, now: SystemTime) -> Option<Purpose> {
        let mut issued = self.0.lock().expect("ticket store is never poisoned");
        let found = issued.get(ticket)?;
        let live = found.expires > now;
        let valid = live && found.admission == *admission;
        if live && !valid && !matches!(found.purpose, Purpose::Terminal(_)) {
            return None;
        }
        let spent = issued.remove(ticket).expect("the ticket was just found");
        valid.then_some(spent.purpose)
    }
}

#[cfg(test)]
mod tests {
    use super::{Admission, Purpose, Tickets, TICKET_TTL};
    use crate::id::LfProcessId;
    use crate::transport::Host;
    use std::time::{Duration, SystemTime};

    fn download(text: &str) -> Purpose {
        let (outbox, _queue) = super::super::outbox::Outbox::new();
        Purpose::Download {
            message: text.into(),
            reservation: outbox.reserve().unwrap(),
        }
    }

    fn terminal() -> Purpose {
        Purpose::Terminal(Host::Owner(LfProcessId::new()))
    }

    fn remote(node: &str, user: u64) -> Admission {
        Admission::Remote {
            node: node.into(),
            user,
        }
    }

    #[test]
    fn a_ticket_opens_one_bridge_for_its_user_within_its_lifetime() {
        let tickets = Tickets::default();
        let phone = remote("phone", 7);
        let ticket = tickets.issue(phone.clone(), terminal());
        assert!(matches!(
            tickets.redeem(&ticket, &phone),
            Some(Purpose::Terminal(_))
        ));
        assert!(tickets.redeem(&ticket, &phone).is_none(), "single use");
        assert!(tickets.redeem("unknown", &phone).is_none());
    }

    #[test]
    fn any_foreign_attempt_spends_a_terminal_ticket() {
        let tickets = Tickets::default();
        let phone = remote("phone", 7);
        for (owner, attempt) in [
            (phone.clone(), remote("laptop", 7)),
            (phone.clone(), remote("phone", 8)),
            (phone.clone(), Admission::Local),
            (Admission::Local, phone.clone()),
        ] {
            let ticket = tickets.issue(owner.clone(), terminal());
            assert!(tickets.redeem(&ticket, &attempt).is_none());
            assert!(
                tickets.redeem(&ticket, &owner).is_none(),
                "{attempt:?} spent a ticket issued to {owner:?}"
            );
        }
    }

    #[test]
    fn a_bulk_ticket_survives_another_devices_attempt() {
        let tickets = Tickets::default();
        let phone = remote("phone", 7);
        let ticket = tickets.issue(phone.clone(), download("body"));
        assert!(tickets.redeem(&ticket, &Admission::Local).is_none());
        assert!(tickets.redeem(&ticket, &remote("laptop", 7)).is_none());
        assert!(matches!(
            tickets.redeem(&ticket, &phone),
            Some(Purpose::Download { message, .. }) if message == "body"
        ));
        assert!(tickets.redeem(&ticket, &phone).is_none(), "single use");
    }

    #[test]
    fn host_sleep_counts_toward_a_tickets_sixty_seconds() {
        assert_eq!(TICKET_TTL, Duration::from_secs(60));
        let tickets = Tickets::default();
        let issued = SystemTime::now();
        let fresh = tickets.issue_at(Admission::Local, terminal(), issued);
        let slept = tickets.issue_at(Admission::Local, terminal(), issued);
        let almost = issued + TICKET_TTL - Duration::from_secs(1);
        assert!(tickets
            .redeem_at(&fresh, &Admission::Local, almost)
            .is_some());
        // The wall clock moved past the lifetime while the host slept.
        let woke = issued + TICKET_TTL + Duration::from_secs(1);
        assert!(tickets.redeem_at(&slept, &Admission::Local, woke).is_none());
        assert!(
            tickets.0.lock().unwrap().is_empty(),
            "expired ticket removed"
        );
    }

    #[test]
    fn issuing_sweeps_expired_tickets() {
        let tickets = Tickets::default();
        let issued = SystemTime::now();
        let old = tickets.issue_at(Admission::Local, download("body"), issued);
        tickets.issue_at(Admission::Local, download("next"), issued + TICKET_TTL);
        assert!(!tickets.0.lock().unwrap().contains_key(&old));
    }

    #[tokio::test]
    async fn abandoned_download_releases_capacity_without_another_request() {
        let tickets = std::sync::Arc::new(Tickets::default());
        let (outbox, _queue) = super::super::outbox::Outbox::new();
        let ticket = tickets.issue(
            Admission::Local,
            Purpose::Download {
                message: "held".into(),
                reservation: outbox.reserve().unwrap(),
            },
        );
        assert!(outbox.retained() > 0);
        tickets.0.lock().unwrap().get_mut(&ticket).unwrap().expires = SystemTime::now();
        let expire = tickets.clone();
        let task = tokio::spawn(async move { expire.expiring().await });
        tokio::time::timeout(Duration::from_secs(3), async {
            while outbox.retained() != 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(outbox.reserve().is_some());
    }
}
