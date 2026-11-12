//! Tickets admit a second connection for what the control connection must not
//! carry: a terminal's bytes, or one message larger than the control limit.
//! A ticket is single use, bound to the admission that requested it, and
//! expires unused. It grants nothing the control connection did not already have.
use super::outbox::Reservation;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::Engine as _;
use tokio::sync::mpsc;

use super::wire::ClientMessage;
use crate::id::TerminalId;

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
    /// The viewer protocol of a held terminal, copied byte for byte.
    Terminal(TerminalId),
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
    expires: Instant,
}

/// Every ticket the server has issued and not yet redeemed or expired.
#[derive(Debug, Default)]
pub(super) struct Tickets(Mutex<HashMap<String, Ticket>>);

impl Tickets {
    pub(super) fn issue(&self, admission: Admission, purpose: Purpose) -> String {
        let ticket = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(uuid::Uuid::new_v4().as_bytes());
        let mut issued = self.0.lock().expect("ticket store is never poisoned");
        let now = Instant::now();
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
                .retain(|_, ticket| ticket.expires > Instant::now());
        }
    }

    /// The ticket's purpose, once; `None` for an unknown, expired, or foreign ticket.
    pub(super) fn redeem(&self, ticket: &str, admission: &Admission) -> Option<Purpose> {
        let mut issued = self.0.lock().expect("ticket store is never poisoned");
        let valid = issued.get(ticket).is_some_and(|issued| {
            issued.expires > Instant::now() && issued.admission == *admission
        });
        if !valid {
            return None;
        }
        issued.remove(ticket).map(|ticket| ticket.purpose)
    }
}

#[cfg(test)]
mod tests {
    use super::{Admission, Purpose, Tickets};

    fn download(text: &str) -> Purpose {
        let (outbox, _queue) = super::super::outbox::Outbox::new();
        Purpose::Download {
            message: text.into(),
            reservation: outbox.reserve().unwrap(),
        }
    }

    #[test]
    fn expired_tickets_cannot_be_redeemed() {
        let tickets = Tickets::default();
        let ticket = tickets.issue(Admission::Local, download("body"));
        tickets.0.lock().unwrap().get_mut(&ticket).unwrap().expires = std::time::Instant::now();
        assert!(tickets.redeem(&ticket, &Admission::Local).is_none());
        tickets.issue(Admission::Local, download("next"));
        assert!(!tickets.0.lock().unwrap().contains_key(&ticket));
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
        tickets.0.lock().unwrap().get_mut(&ticket).unwrap().expires = std::time::Instant::now();
        let expire = tickets.clone();
        let task = tokio::spawn(async move { expire.expiring().await });
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while outbox.retained() != 0 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(outbox.reserve().is_some());
    }

    #[test]
    fn tickets_are_single_use_and_bound_to_their_admission() {
        let tickets = Tickets::default();
        let phone = Admission::Remote {
            node: "n1".into(),
            user: 7,
        };
        let ticket = tickets.issue(phone.clone(), download("body"));
        assert!(tickets.redeem(&ticket, &Admission::Local).is_none());
        let other = Admission::Remote {
            node: "n2".into(),
            user: 7,
        };
        assert!(tickets.redeem(&ticket, &other).is_none());
        assert!(matches!(
            tickets.redeem(&ticket, &phone),
            Some(Purpose::Download { message, .. }) if message == "body"
        ));
        assert!(tickets.redeem(&ticket, &phone).is_none(), "single use");
        assert!(tickets.redeem("unknown", &phone).is_none());
    }
}
