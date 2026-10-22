//! Machines you own and machines you rent.
//!
//! A **Pool** groups machines you own; it knows no provider. An **OnDemand**
//! is what you rent: a provider template and a budget (how many at once, the
//! most per hour). They share one namespace. A **Rental** is one paid window
//! on a provider: its price, hardware, cap, deadline and cost. It becomes an
//! ordinary Machine once reachable; Task execution never learns it is rented.
//!
//! Admission writes the Rental's identity before the provider is asked, so an
//! unknown reply keeps it; the create is never resent. The pod's guardian
//! deletes it at the hard deadline whatever else fails.

pub mod runpod;

use std::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::durable::{DurableDataError, MachineId, TaskId};
use crate::id::prefixed_uuid_id;

prefixed_uuid_id!(
    RentalId,
    "rent_",
    DurableDataError,
    DurableDataError::InvalidId
);

/// How long a rented machine lasts when its on-demand names no other window.
pub const DEFAULT_HOURS: f64 = 4.0;
/// Before the hard deadline, give back now rather than when drained.
pub const SOFT_DEADLINE_SECONDS: i64 = 15 * 60;
/// An unconfirmed create absent from the inventory after this long is lost.
pub const CREATE_SECONDS: i64 = 5 * 60;
/// LOO-554: RunPod's stock Ubuntu image, pinned by digest when an on-demand is added.
pub const STOCK_IMAGE: &str = "runpod/base:1.0.2-ubuntu2404";

// ---------------------------------------------------------------------------
// Money
// ---------------------------------------------------------------------------

/// US dollars in millionths, so prices like $0.03 per vCPU-hour stay exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Usd(i64);

impl Usd {
    pub const ZERO: Usd = Usd(0);

    pub fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    pub fn micros(self) -> i64 {
        self.0
    }

    /// Parse `0.40`, `$0.40` or `3`; at most six decimal places.
    pub fn parse(value: &str) -> Result<Self, String> {
        let text = value.trim().trim_start_matches('$');
        let invalid = || format!("{value:?} is not a dollar amount");
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        if whole.is_empty() && fraction.is_empty()
            || !whole.chars().all(|c| c.is_ascii_digit())
            || !fraction.chars().all(|c| c.is_ascii_digit())
            || fraction.len() > 6
        {
            return Err(invalid());
        }
        let whole: i64 = if whole.is_empty() {
            0
        } else {
            whole.parse().map_err(|_| invalid())?
        };
        let fraction: i64 = format!("{fraction:0<6}").parse().map_err(|_| invalid())?;
        whole
            .checked_mul(1_000_000)
            .and_then(|micros| micros.checked_add(fraction))
            .map(Self)
            .ok_or_else(invalid)
    }

    /// Exact decimal text for storage.
    pub fn to_decimal(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let micros = self.0.unsigned_abs();
        let fraction = format!("{:06}", micros % 1_000_000);
        let fraction = fraction.trim_end_matches('0');
        let fraction = format!("{fraction:0<2}");
        format!("{sign}{}.{fraction}", micros / 1_000_000)
    }

    pub fn times(self, count: i64) -> Self {
        Self(self.0.saturating_mul(count))
    }

    /// This hourly price over `hours`, rounded down to the millionth.
    pub fn over_hours(self, hours: f64) -> Self {
        Self((self.0 as f64 * hours).floor() as i64)
    }

    /// Hours this budget buys at `per_hour`.
    pub fn hours_at(self, per_hour: Usd) -> Option<f64> {
        (per_hour.0 > 0).then(|| self.0 as f64 / per_hour.0 as f64)
    }
}

impl fmt::Display for Usd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.to_decimal())
    }
}

/// Serialized as exact decimal text, as stored.
impl Serialize for Usd {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_decimal())
    }
}

impl std::str::FromStr for Usd {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

// ---------------------------------------------------------------------------
// Templates and hardware
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Provider {
    Runpod,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Runpod => "runpod",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "runpod" => Some(Self::Runpod),
            _ => None,
        }
    }
}

/// What an on-demand asks a provider for. No GPU rents the cheapest CPU pod
/// covering `vcpus` and `memory_gb`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    pub gpu: Option<String>,
    pub vcpus: u32,
    pub memory_gb: u32,
    pub disk_gb: Option<u32>,
    /// Pinned by digest when the on-demand is added.
    pub image: String,
    pub data_center: Option<String>,
}

/// What was priced, asked for and recorded: create asks for exactly this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Hardware {
    Cpu {
        flavor: String,
        vcpus: u32,
    },
    Gpu {
        id: String,
        min_vcpus: u32,
        min_memory_gb: u32,
    },
}

impl Hardware {
    /// Container disk: 40 GB, or 10 GB per vCPU on CPU flavors that cap it.
    pub fn disk_gb(&self) -> u32 {
        match self {
            Self::Gpu { .. } => 40,
            Self::Cpu { vcpus, .. } => (10 * vcpus).min(40),
        }
    }
}

impl fmt::Display for Hardware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu { flavor, vcpus } => write!(f, "{flavor}×{vcpus}"),
            Self::Gpu { id, .. } => f.write_str(id),
        }
    }
}

/// What a provider would rent now, at what price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    pub hardware: Hardware,
    pub usd_per_hour: Usd,
    /// False only when the catalog positively reports none.
    pub available: bool,
}

/// One CPU flavor from a provider catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct CpuFlavor {
    pub id: String,
    pub min_vcpus: u32,
    pub max_vcpus: u32,
    pub ram_gb_per_vcpu: f64,
    pub usd_per_vcpu_hour: Usd,
    pub available: bool,
}

/// CPU pods come in power-of-two vCPU counts with memory a fixed multiple of
/// them. Take the smallest size covering both needs in each flavor, then the
/// cheapest, preferring flavors in stock.
pub fn cpu_quote(flavors: &[CpuFlavor], vcpus: u32, memory_gb: u32) -> Option<Quote> {
    let smallest = vcpus.max(2).next_power_of_two();
    flavors
        .iter()
        .filter_map(|flavor| {
            let mut size = smallest.max(flavor.min_vcpus);
            while (size as f64) * flavor.ram_gb_per_vcpu < memory_gb as f64 {
                size = size.checked_mul(2)?;
            }
            let usd_per_hour = flavor.usd_per_vcpu_hour.times(size as i64);
            (size <= flavor.max_vcpus && usd_per_hour > Usd::ZERO).then(|| Quote {
                hardware: Hardware::Cpu {
                    flavor: flavor.id.clone(),
                    vcpus: size,
                },
                usd_per_hour,
                available: flavor.available,
            })
        })
        .min_by_key(|quote| (!quote.available, quote.usd_per_hour))
}

/// The smallest power-of-two vCPU count RunPod's catalog is asked about.
pub fn catalog_vcpus(vcpus: u32) -> u32 {
    vcpus.max(2).next_power_of_two()
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pool {
    pub name: String,
    /// Takes no new work; what it holds stays.
    pub off: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OnDemand {
    pub name: String,
    pub provider: Provider,
    pub template: Template,
    pub max_machines: u32,
    pub max_usd_per_hour: Usd,
    pub hours: f64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// An owned machine's commitments: its pool, what it cost, when it stops
/// taking new work. A rented machine has none of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ownership {
    pub pool: Option<String>,
    pub cost_usd: Usd,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RentalState {
    /// Written before the create; no reply yet.
    Requested,
    /// The create's reply was lost; the provider name finds it.
    Unconfirmed,
    /// The provider confirmed the pod; not reachable yet.
    Booting,
    /// Added as a Machine.
    Running,
    /// Giving back has begun.
    Ending,
    Ended,
    /// Never found under its name after the create's deadline.
    Lost,
}

impl RentalState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Unconfirmed => "unconfirmed",
            Self::Booting => "booting",
            Self::Running => "running",
            Self::Ending => "ending",
            Self::Ended => "ended",
            Self::Lost => "lost",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "requested" => Self::Requested,
            "unconfirmed" => Self::Unconfirmed,
            "booting" => Self::Booting,
            "running" => Self::Running,
            "ending" => Self::Ending,
            "ended" => Self::Ended,
            "lost" => Self::Lost,
            _ => return None,
        })
    }

    /// Counts against the on-demand's limit and may be costing money.
    pub fn is_live(self) -> bool {
        !matches!(self, Self::Ended | Self::Lost)
    }
}

impl fmt::Display for RentalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rental {
    pub id: RentalId,
    pub ondemand: String,
    pub provider: Provider,
    pub provider_name: String,
    pub provider_id: Option<String>,
    pub machine_id: Option<MachineId>,
    /// Lasts until its deadline, its cap or an explicit end; otherwise it ends
    /// when its work drains.
    pub by_hand: bool,
    pub hardware: Hardware,
    pub usd_per_hour: Usd,
    pub cap_usd: Usd,
    #[serde(with = "time::serde::rfc3339")]
    pub deadline: OffsetDateTime,
    pub state: RentalState,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ended_at: Option<OffsetDateTime>,
    pub cost_usd: Option<Usd>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub copied_at: Option<OffsetDateTime>,
}

impl Rental {
    /// The pod's name at the provider: our identity, sent before any reply.
    pub fn provider_name_for(id: &RentalId) -> String {
        format!("lf-{id}")
    }

    pub fn soft_deadline(&self) -> OffsetDateTime {
        self.deadline - time::Duration::seconds(SOFT_DEADLINE_SECONDS)
    }
}

// ---------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------

/// What admission grants: exactly this hardware, at most this cap, until this deadline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    pub hardware: Hardware,
    pub usd_per_hour: Usd,
    pub cap_usd: Usd,
    pub deadline: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Refusal {
    #[error("{ondemand} already has {live} of {max} machines; `lf ondemand max {ondemand} N` raises the limit")]
    AtLimit {
        ondemand: String,
        live: u32,
        max: u32,
    },
    #[error("no {hardware} available for {ondemand} now")]
    Unavailable { ondemand: String, hardware: String },
    #[error("{ondemand}: {hardware} costs {seen}/h, above its {max}/h limit")]
    OverPrice {
        ondemand: String,
        hardware: String,
        seen: Usd,
        max: Usd,
    },
}

/// Etude's order: the machine limit, then stock and price, then the cap.
pub fn admit(
    ondemand: &OnDemand,
    live: u32,
    quote: &Quote,
    hours: Option<f64>,
    max_usd: Option<Usd>,
    now: OffsetDateTime,
) -> Result<Admission, Refusal> {
    if live >= ondemand.max_machines {
        return Err(Refusal::AtLimit {
            ondemand: ondemand.name.clone(),
            live,
            max: ondemand.max_machines,
        });
    }
    if !quote.available {
        return Err(Refusal::Unavailable {
            ondemand: ondemand.name.clone(),
            hardware: quote.hardware.to_string(),
        });
    }
    if quote.usd_per_hour > ondemand.max_usd_per_hour {
        return Err(Refusal::OverPrice {
            ondemand: ondemand.name.clone(),
            hardware: quote.hardware.to_string(),
            seen: quote.usd_per_hour,
            max: ondemand.max_usd_per_hour,
        });
    }
    let hours = hours.unwrap_or(ondemand.hours);
    let window = quote.usd_per_hour.over_hours(hours);
    let cap_usd = max_usd.map_or(window, |max| max.min(window));
    let paid = cap_usd
        .hours_at(quote.usd_per_hour)
        .unwrap_or(hours)
        .min(hours);
    Ok(Admission {
        hardware: quote.hardware.clone(),
        usd_per_hour: quote.usd_per_hour,
        cap_usd,
        deadline: now + time::Duration::seconds((paid * 3600.0) as i64),
    })
}

/// What an unconfirmed create turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution<P> {
    Adopt(P),
    Lost,
    /// Still within the create's own deadline; look again later.
    Wait,
}

pub fn resolve_unconfirmed<P>(
    found: Option<P>,
    created_at: OffsetDateTime,
    now: OffsetDateTime,
) -> Resolution<P> {
    match found {
        Some(pod) => Resolution::Adopt(pod),
        None if now - created_at >= time::Duration::seconds(CREATE_SECONDS) => Resolution::Lost,
        None => Resolution::Wait,
    }
}

// ---------------------------------------------------------------------------
// Pools
// ---------------------------------------------------------------------------

/// An owned machine as placement sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PoolMachine {
    pub machine: crate::durable::Machine,
    pub ownership: Ownership,
    /// Unfinished Tasks this machine owns.
    pub tasks: Vec<TaskId>,
}

impl PoolMachine {
    pub fn expired(&self, now: OffsetDateTime) -> bool {
        self.ownership.expires_at.is_some_and(|at| at <= now)
    }

    pub fn label(&self) -> &str {
        self.machine
            .label
            .as_deref()
            .unwrap_or(self.machine.id.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PoolRefusal {
    #[error("pool {0} is off; `lf pool on {0}` lets it take new work")]
    Off(String),
    #[error("pool {0} has no machines; `lf machine add <dest> --pool {0}` adds one")]
    Empty(String),
    #[error("pool {pool} has no free machine: {}", busy.join(", "))]
    Busy { pool: String, busy: Vec<String> },
}

/// The first machine in the pool holding no unfinished Task and not expired.
/// Loopflow keeps no queue; waiting is the person's choice.
pub fn free_machine<'a>(
    pool: &Pool,
    machines: &'a [PoolMachine],
    now: OffsetDateTime,
) -> Result<&'a PoolMachine, PoolRefusal> {
    if pool.off {
        return Err(PoolRefusal::Off(pool.name.clone()));
    }
    if machines.is_empty() {
        return Err(PoolRefusal::Empty(pool.name.clone()));
    }
    machines
        .iter()
        .find(|m| !m.expired(now) && m.tasks.is_empty())
        .ok_or_else(|| PoolRefusal::Busy {
            pool: pool.name.clone(),
            busy: machines
                .iter()
                .map(|m| {
                    if m.expired(now) {
                        format!("{} expired", m.label())
                    } else {
                        let tasks = m.tasks.iter().map(|t| t.to_string()).collect::<Vec<_>>();
                        format!("{} holds {}", m.label(), tasks.join(" "))
                    }
                })
                .collect(),
        })
}

// ---------------------------------------------------------------------------
// Giving back
// ---------------------------------------------------------------------------

/// What the rented machine holds right now, read over SSH and from the
/// ownership log.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Holdings {
    /// It has been handed a Task; until then it is still being prepared.
    pub placed: bool,
    pub tasks: Vec<TaskId>,
    pub live_processes: usize,
    /// Worktrees with uncommitted changes or commits not on their upstream.
    pub unpushed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiveBack {
    /// Keep it: work is still there and no deadline forces the end.
    Keep(String),
    /// The pod holds the only copy of this work; never delete it.
    Refuse(Vec<String>),
    /// Copy records up, then delete.
    End,
}

/// Drain for Task rentals, the soft deadline for every rental, and an explicit
/// end. Unpushed work refuses every one of them unless forced.
pub fn give_back(
    rental: &Rental,
    holdings: &Holdings,
    explicit: bool,
    force: bool,
    now: OffsetDateTime,
) -> GiveBack {
    let due = explicit || now >= rental.soft_deadline();
    let drained = holdings.placed && holdings.tasks.is_empty() && holdings.live_processes == 0;
    if !due && (rental.by_hand || !drained) {
        let reason = if rental.by_hand {
            format!("rented by hand until {}", rental.deadline)
        } else {
            format!(
                "{} Task(s) and {} live Process(es) remain",
                holdings.tasks.len(),
                holdings.live_processes
            )
        };
        return GiveBack::Keep(reason);
    }
    if !holdings.unpushed.is_empty() && !force {
        return GiveBack::Refuse(holdings.unpushed.clone());
    }
    GiveBack::End
}

#[cfg(test)]
mod tests;
