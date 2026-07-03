//! Money and spend-cap types for wave budgets.
//!
//! `Money` is a cents newtype so budget arithmetic never touches floats.
//! Agent harnesses report cost as USD (`f64`); we convert once at the boundary
//! and account in integer cents thereafter.

use serde::{Deserialize, Serialize};

/// A monetary amount in whole US cents. Wire shape: a bare integer.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, Hash,
)]
#[serde(transparent)]
pub struct Money {
    cents: i64,
}

impl Money {
    pub const ZERO: Money = Money { cents: 0 };

    pub fn from_cents(cents: i64) -> Self {
        Self { cents }
    }

    /// Convert a USD amount (as reported by an agent harness) into cents,
    /// rounding to the nearest cent. Negative or NaN inputs clamp to zero.
    pub fn from_usd(usd: f64) -> Self {
        if !usd.is_finite() || usd <= 0.0 {
            return Self::ZERO;
        }
        Self {
            cents: (usd * 100.0).round() as i64,
        }
    }

    pub fn cents(&self) -> i64 {
        self.cents
    }

    pub fn as_usd(&self) -> f64 {
        self.cents as f64 / 100.0
    }

    pub fn is_zero(&self) -> bool {
        self.cents == 0
    }

    pub fn saturating_add(self, other: Money) -> Money {
        Money {
            cents: self.cents.saturating_add(other.cents),
        }
    }

    /// Headroom remaining under `cap`; zero once spend meets or exceeds it.
    pub fn headroom_under(self, cap: Money) -> Money {
        Money {
            cents: (cap.cents - self.cents).max(0),
        }
    }
}

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "${:.2}", self.as_usd())
    }
}

/// A wave's hard spend ceiling. `rate` is the cumulative ceiling for the wave's
/// activity; `per_iteration` catches a single pathological run. Both in cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendCap {
    /// Cumulative spend ceiling. Crossing it pauses the wave and blocks to human.
    pub rate: Money,
    /// Ceiling for one iteration's cost. A single run above it pauses+blocks.
    pub per_iteration: Money,
}

impl SpendCap {
    /// Whether cumulative `spent` has reached or crossed the cumulative ceiling.
    pub fn rate_exceeded(&self, spent: Money) -> bool {
        !self.rate.is_zero() && spent >= self.rate
    }

    /// Whether a single iteration's `cost` breaches the per-iteration ceiling.
    pub fn iteration_exceeded(&self, cost: Money) -> bool {
        !self.per_iteration.is_zero() && cost > self.per_iteration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_usd_rounds_to_cents() {
        assert_eq!(Money::from_usd(0.05).cents(), 5);
        assert_eq!(Money::from_usd(1.239).cents(), 124);
        assert_eq!(Money::from_usd(0.0).cents(), 0);
    }

    #[test]
    fn negative_and_nan_clamp_to_zero() {
        assert_eq!(Money::from_usd(-1.0), Money::ZERO);
        assert_eq!(Money::from_usd(f64::NAN), Money::ZERO);
    }

    #[test]
    fn money_serializes_as_bare_integer() {
        let json = serde_json::to_string(&Money::from_cents(4200)).expect("serialize");
        assert_eq!(json, "4200");
        let parsed: Money = serde_json::from_str("4200").expect("parse");
        assert_eq!(parsed, Money::from_cents(4200));
    }

    #[test]
    fn headroom_never_negative() {
        let spent = Money::from_cents(150);
        assert_eq!(spent.headroom_under(Money::from_cents(100)), Money::ZERO);
        assert_eq!(
            Money::from_cents(30).headroom_under(Money::from_cents(100)),
            Money::from_cents(70)
        );
    }

    #[test]
    fn rate_exceeded_ignores_zero_cap() {
        let cap = SpendCap {
            rate: Money::ZERO,
            per_iteration: Money::from_cents(100),
        };
        assert!(!cap.rate_exceeded(Money::from_cents(9999)));
    }

    #[test]
    fn rate_and_iteration_thresholds() {
        let cap = SpendCap {
            rate: Money::from_cents(500),
            per_iteration: Money::from_cents(100),
        };
        assert!(!cap.rate_exceeded(Money::from_cents(499)));
        assert!(cap.rate_exceeded(Money::from_cents(500)));
        assert!(!cap.iteration_exceeded(Money::from_cents(100)));
        assert!(cap.iteration_exceeded(Money::from_cents(101)));
    }

    #[test]
    fn spend_cap_wire_shape() {
        let cap = SpendCap {
            rate: Money::from_cents(5000),
            per_iteration: Money::from_cents(1000),
        };
        let json = serde_json::to_value(cap).expect("serialize");
        assert_eq!(json["rate"], 5000);
        assert_eq!(json["per_iteration"], 1000);
    }
}
