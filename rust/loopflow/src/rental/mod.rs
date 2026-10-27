//! Machines you own and machines you rent.
//!
//! A **Pool** groups machines you own; it knows no provider. An **OnDemand**
//! is what you rent: a provider, hardware and a budget (how many at once, the
//! most per hour). They share one namespace. A **Rental** is one paid window
//! on a provider: its price, hardware, cap, deadline and cost. It becomes an
//! ordinary Machine once reachable; Task execution never learns it is rented.
//!
//! Admission writes the Rental's identity before the provider is asked, so an
//! unknown reply keeps it; the create is never resent. At the hard deadline
//! the provider deletes the machine whatever else fails: its own lifetime
//! (Daytona, Modal) or the guardian on the machine (`guardian.py`: RunPod,
//! AWS).

pub mod sandbox;

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
/// After its last Task finishes, a rental waits this long before going back.
pub const GRACE_SECONDS: i64 = 15 * 60;
/// The longest window a rental lasts (Modal's limit for a sandbox).
pub const MAX_HOURS: f64 = 24.0;
/// An unconfirmed create absent under its name after this long is lost.
pub const CREATE_SECONDS: i64 = 20 * 60;
/// A rental still booting this long after admission lost the lf booting it
/// (its create, SSH wait and install each fit well inside); nothing ran on it.
pub const BOOT_SECONDS: i64 = 60 * 60;

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

impl std::ops::Add for Usd {
    type Output = Usd;
    fn add(self, other: Usd) -> Usd {
        Usd(self.0.saturating_add(other.0))
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

/// Read from exact decimal text, as the rental bridge reports a price.
impl<'de> Deserialize<'de> for Usd {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl std::str::FromStr for Usd {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

// ---------------------------------------------------------------------------
// Providers and hardware
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Provider {
    Daytona,
    Modal,
    Runpod,
    Aws,
    /// Retired 2026-10-27: its disk cannot hold a Rust build. 0.14.17's Fly
    /// on-demands and rentals stay readable as history; nothing rents there.
    #[value(skip)]
    Fly,
}

const RETIRED: &str = "Fly no longer rents; `lf ondemand remove` forgets its on-demand";

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daytona => "daytona",
            Self::Modal => "modal",
            Self::Runpod => "runpod",
            Self::Aws => "aws",
            Self::Fly => "fly",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "daytona" => Some(Self::Daytona),
            "modal" => Some(Self::Modal),
            "runpod" => Some(Self::Runpod),
            "aws" => Some(Self::Aws),
            "fly" => Some(Self::Fly),
            _ => None,
        }
    }

    /// The SDK the bridge runs with, pinned; RunPod speaks plain HTTP.
    pub fn sdk(self) -> Option<&'static str> {
        match self {
            Self::Daytona => Some("daytona==0.226.1"),
            Self::Modal => Some("modal==1.6.2"),
            Self::Aws => Some("boto3==1.40.55"),
            Self::Runpod | Self::Fly => None,
        }
    }

    /// Why this lf process cannot reach the provider's account, if it cannot.
    pub fn missing_credentials(self) -> Option<String> {
        let set = |name: &str| std::env::var(name).is_ok_and(|value| !value.is_empty());
        let doppler = |names: &str| format!("{names} not set; run with `doppler run -- lf …`");
        match self {
            Self::Daytona if !set("DAYTONA_API_KEY") => Some(doppler("DAYTONA_API_KEY is")),
            Self::Modal
                if !set("MODAL_TOKEN_ID")
                    && !dirs::home_dir().is_some_and(|home| home.join(".modal.toml").is_file()) =>
            {
                Some(
                    "no Modal token: set MODAL_TOKEN_ID and MODAL_TOKEN_SECRET, or `uvx modal token new`"
                        .to_string(),
                )
            }
            Self::Fly => Some(RETIRED.to_string()),
            Self::Runpod if !set("RUNPOD_API_KEY") => Some(doppler("RUNPOD_API_KEY is")),
            Self::Aws
                if !set("AWS_ACCESS_KEY_ID")
                    || !set("AWS_SECRET_ACCESS_KEY")
                    || !(set("AWS_REGION") || set("AWS_DEFAULT_REGION")) =>
            {
                Some(doppler(
                    "AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY and AWS_REGION are not all",
                ))
            }
            _ => None,
        }
    }

    /// What this provider rents for the hardware, at its published price.
    /// Where it sells fixed shapes (RunPod CPU flavors, EC2 instance types),
    /// the smallest one holding the hardware.
    pub fn offer(self, hardware: &Hardware) -> Result<Offer, String> {
        let vcpus = i64::from(hardware.vcpus);
        let memory = i64::from(hardware.memory_gb);
        let offer = |machine_type: Option<String>, micros: i64| Offer {
            machine_type,
            usd_per_hour: Usd::from_micros(micros),
        };
        let per_size = |vcpu: i64, gib: i64, extra: i64| vcpu * vcpus + gib * memory + extra;
        let no_shape = || format!("{self} rents nothing with {hardware}");
        if hardware.spot && !matches!(self, Self::Aws | Self::Runpod) {
            return Err(format!(
                "{self} sells no interruptible capacity; --spot works on aws and runpod"
            ));
        }
        match (self, hardware.gpu.as_deref()) {
            (Self::Daytona, Some(_)) => Err(format!(
                "{self} rents no GPUs; --gpu works on runpod, aws and modal"
            )),
            (Self::Daytona, None) => Ok(offer(
                None,
                per_size(
                    50_400,
                    16_200,
                    108 * i64::from(hardware.disk_gb.saturating_sub(5)),
                ),
            )),
            (Self::Modal, None) => Ok(offer(None, per_size(70_956, 24_012, 0))),
            (Self::Modal, Some(gpu)) => {
                let (name, usd) = find_gpu(self, MODAL_GPUS, gpu)?;
                Ok(offer(
                    Some(name.to_string()),
                    per_size(70_956, 24_012, *usd),
                ))
            }
            (Self::Runpod, None) => RUNPOD_CPUS
                .iter()
                .find(|(_, gib_per_vcpu, _)| gib_per_vcpu * vcpus >= memory)
                .map(|(flavor, _, usd)| offer(Some((*flavor).to_string()), usd * vcpus))
                .ok_or_else(no_shape),
            (Self::Runpod, Some(gpu)) => {
                let (id, usd) = find_gpu(self, RUNPOD_GPUS, gpu)?;
                Ok(offer(Some(id.to_string()), *usd))
            }
            (Self::Aws, None) => AWS_CPUS
                .iter()
                .find(|(_, gib_per_vcpu, _)| gib_per_vcpu * vcpus >= memory)
                .and_then(|(family, gib_per_vcpu, usd)| {
                    let (size_vcpus, size) = AWS_SIZES
                        .iter()
                        .find(|(size_vcpus, _)| *size_vcpus >= vcpus)?;
                    (gib_per_vcpu * size_vcpus >= memory)
                        .then(|| offer(Some(format!("{family}.{size}")), usd * size_vcpus))
                })
                .ok_or_else(no_shape),
            (Self::Fly, _) => Err(RETIRED.to_string()),
            (Self::Aws, Some(gpu)) => {
                let (family, sizes) = find_gpu(self, AWS_GPUS, gpu)?;
                sizes
                    .iter()
                    .find(|(size_vcpus, size_gib, _, _)| {
                        *size_vcpus >= vcpus && *size_gib >= memory
                    })
                    .map(|(_, _, size, usd)| offer(Some(format!("{family}.{size}")), *usd))
                    .ok_or_else(no_shape)
            }
        }
    }

    pub fn usd_per_hour(self, hardware: &Hardware) -> Result<Usd, String> {
        self.offer(hardware).map(|offer| offer.usd_per_hour)
    }

    /// Where it rents, set in the environment, and whose prices `offer` read
    /// when they differ: EC2's table is us-east-1, other regions charge
    /// otherwise and report no price. `None` where lf picks no region.
    pub fn region(self) -> Option<String> {
        let priced = "us-east-1";
        match self {
            Self::Aws => {
                let region = ["AWS_REGION", "AWS_DEFAULT_REGION"]
                    .iter()
                    .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))?;
                Some(if region == priced {
                    region
                } else {
                    format!("{region}, priced as {priced}")
                })
            }
            _ => None,
        }
    }
}

/// What a provider rents for some hardware.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The provider's own name for it: a RunPod CPU flavor or GPU type, an EC2
    /// instance type, a Modal GPU. `None` where the hardware is the request.
    pub machine_type: Option<String>,
    pub usd_per_hour: Usd,
}

// Published prices in millionths of a dollar per hour, 2026-10-26. Daytona
// bills $0.0504/vCPU-h, $0.0162/GiB-h and $0.000108/GiB-h of disk past 5; Modal
// sandboxes $0.00003942/core-s (two vCPUs a core) and $0.00000667/GiB-s.
// RunPod and EC2 reply with the price they charge; these admit before asking.

/// `--gpu` name, Modal's name, its price on top of the sandbox's CPU and memory.
const MODAL_GPUS: &[(&str, &str, i64)] = &[
    ("T4", "T4", 590_000),
    ("L4", "L4", 800_000),
    ("A10G", "A10G", 1_100_000),
    ("L40S", "L40S", 1_950_000),
    ("A100", "A100-80GB", 2_500_000),
    ("H100", "H100", 3_950_000),
];

/// Secure-cloud pods with one GPU: `--gpu` name, RunPod's id, price.
const RUNPOD_GPUS: &[(&str, &str, i64)] = &[
    ("A40", "NVIDIA A40", 590_000),
    ("A6000", "NVIDIA RTX A6000", 590_000),
    ("L4", "NVIDIA L4", 590_000),
    ("RTX4090", "NVIDIA GeForce RTX 4090", 890_000),
    ("L40S", "NVIDIA L40S", 1_090_000),
    ("A100", "NVIDIA A100 80GB PCIe", 1_790_000),
    ("H100", "NVIDIA H100 PCIe", 2_890_000),
];

/// RunPod CPU flavors: GiB per vCPU, an estimated price per vCPU (RunPod
/// publishes none; the create reports what it charges).
const RUNPOD_CPUS: &[(&str, i64, i64)] = &[
    ("cpu3c", 2, 30_000),
    ("cpu3g", 4, 40_000),
    ("cpu3m", 8, 50_000),
];

/// EC2 families by GiB per vCPU, on-demand in us-east-1, per vCPU.
const AWS_CPUS: &[(&str, i64, i64)] = &[("c7i", 2, 44_625), ("m7i", 4, 50_400), ("r7i", 8, 66_150)];

/// EC2 sizes by vCPUs.
const AWS_SIZES: &[(i64, &str)] = &[
    (2, "large"),
    (4, "xlarge"),
    (8, "2xlarge"),
    (16, "4xlarge"),
    (32, "8xlarge"),
    (48, "12xlarge"),
    (64, "16xlarge"),
];

/// EC2 GPU families, one GPU each: vCPUs, GiB, size, on-demand price in us-east-1.
type GpuSizes = &'static [(i64, i64, &'static str, i64)];
const AWS_GPUS: &[(&str, &str, GpuSizes)] = &[
    (
        "L4",
        "g6",
        &[
            (4, 16, "xlarge", 804_800),
            (8, 32, "2xlarge", 977_600),
            (16, 64, "4xlarge", 1_323_200),
        ],
    ),
    (
        "A10G",
        "g5",
        &[
            (4, 16, "xlarge", 1_006_000),
            (8, 32, "2xlarge", 1_212_000),
            (16, 64, "4xlarge", 1_624_000),
        ],
    ),
];

/// The provider's name for a `--gpu` name and what its table holds beside it.
fn find_gpu<T>(
    provider: Provider,
    table: &'static [(&'static str, &'static str, T)],
    gpu: &str,
) -> Result<(&'static str, &'static T), String> {
    table
        .iter()
        .find(|(name, _, _)| *name == gpu)
        .map(|(_, id, rest)| (*id, rest))
        .ok_or_else(|| {
            let known: Vec<&str> = table.iter().map(|(name, _, _)| *name).collect();
            format!("{provider} rents no {gpu}; it rents {}", known.join(", "))
        })
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What an on-demand rents and a rental was granted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hardware {
    pub vcpus: u32,
    pub memory_gb: u32,
    pub disk_gb: u32,
    /// One GPU, by its `--gpu` name; absent from hardware stored without one.
    pub gpu: Option<String>,
    /// Interruptible capacity (AWS spot, RunPod interruptible pods), bid at
    /// the admitted price; the provider may end it before its deadline.
    pub spot: bool,
}

impl fmt::Display for Hardware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} vCPU {} GB", self.vcpus, self.memory_gb)?;
        if let Some(gpu) = &self.gpu {
            write!(f, " + {gpu}")?;
        }
        if self.spot {
            f.write_str(" spot")?;
        }
        Ok(())
    }
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
    pub hardware: Hardware,
    pub max_machines: u32,
    pub max_usd_per_hour: Usd,
    pub hours: f64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl OnDemand {
    pub fn usd_per_hour(&self) -> Result<Usd, String> {
        self.provider.usd_per_hour(&self.hardware)
    }
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
    /// The provider confirmed the sandbox; not reachable yet.
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
}

impl Rental {
    /// The sandbox's name at the provider: our identity, sent before any reply.
    pub fn provider_name_for(id: &RentalId) -> String {
        format!("lf-{id}")
    }

    /// Billed by the second: the hourly price from admission until `end`.
    pub fn cost_until(&self, end: OffsetDateTime) -> Usd {
        let seconds = (end - self.created_at).whole_seconds().max(0);
        self.usd_per_hour.over_hours(seconds as f64 / 3600.0)
    }

    pub fn soft_deadline(&self) -> OffsetDateTime {
        self.deadline - time::Duration::seconds(SOFT_DEADLINE_SECONDS)
    }

    /// Still booting long after admission: the lf booting it died before it
    /// became a Machine, and nothing ran on it.
    pub fn boot_abandoned(&self, now: OffsetDateTime) -> bool {
        self.state == RentalState::Booting
            && now - self.created_at >= time::Duration::seconds(BOOT_SECONDS)
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
    #[error("{ondemand}: {reason}")]
    NotOffered { ondemand: String, reason: String },
    #[error("{ondemand}: {hardware} costs {seen}/h, above its {max}/h limit")]
    OverPrice {
        ondemand: String,
        hardware: String,
        seen: Usd,
        max: Usd,
    },
}

/// The machine limit, then what the provider offers and its price, then the
/// cap. The window is at most `MAX_HOURS`.
pub fn admit(
    ondemand: &OnDemand,
    live: u32,
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
    let usd_per_hour = ondemand
        .usd_per_hour()
        .map_err(|reason| Refusal::NotOffered {
            ondemand: ondemand.name.clone(),
            reason,
        })?;
    if usd_per_hour > ondemand.max_usd_per_hour {
        return Err(Refusal::OverPrice {
            ondemand: ondemand.name.clone(),
            hardware: ondemand.hardware.to_string(),
            seen: usd_per_hour,
            max: ondemand.max_usd_per_hour,
        });
    }
    let hours = hours.unwrap_or(ondemand.hours).min(MAX_HOURS);
    let window = usd_per_hour.over_hours(hours);
    let cap_usd = max_usd.map_or(window, |max| max.min(window));
    let paid = cap_usd.hours_at(usd_per_hour).unwrap_or(hours).min(hours);
    Ok(Admission {
        hardware: ondemand.hardware.clone(),
        usd_per_hour,
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
        Some(sandbox) => Resolution::Adopt(sandbox),
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
    /// When the last Task it held was completed or abandoned.
    pub finished_at: Option<OffsetDateTime>,
    /// Worktrees with uncommitted changes or commits not on their upstream.
    pub unpushed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiveBack {
    /// Keep it: work is still there and no deadline forces the end.
    Keep(String),
    /// The sandbox holds the only copy of this work; never delete it.
    Refuse(Vec<String>),
    /// Copy records up, then delete.
    End,
}

/// Drain plus the grace for Task rentals, the soft deadline for every rental,
/// and an explicit end. Unpushed work refuses every one of them unless forced.
pub fn give_back(
    rental: &Rental,
    holdings: &Holdings,
    explicit: bool,
    force: bool,
    now: OffsetDateTime,
) -> GiveBack {
    let due = explicit || now >= rental.soft_deadline();
    let drained = holdings.placed && holdings.tasks.is_empty() && holdings.live_processes == 0;
    let grace_ends = holdings
        .finished_at
        .map(|at| at + time::Duration::seconds(GRACE_SECONDS));
    let in_grace = grace_ends.is_some_and(|ends| now < ends);
    if !due && (rental.by_hand || !drained || in_grace) {
        let reason = if rental.by_hand {
            format!("rented by hand until {}", rental.deadline)
        } else if drained {
            format!("idle until {}", grace_ends.expect("in grace has an end"))
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
